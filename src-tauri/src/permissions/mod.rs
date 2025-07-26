use anyhow::Result;
use log::{info, debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub connection_id: String,
    pub device_name: String,
    pub device_info: DeviceInfo,
    pub requested_permissions: Vec<Permission>,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub os: String,
    pub version: String,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    ScreenView,
    InputControl,
    FileTransfer,
    Clipboard,
    AudioAccess,
    SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionResponse {
    Granted {
        permissions: Vec<Permission>,
        duration_minutes: Option<u32>, // None = indefinite
    },
    Denied {
        reason: String,
    },
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub connection_id: String,
    pub permissions: Vec<Permission>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub device_info: DeviceInfo,
}

#[derive(Debug, Clone)]
pub enum PermissionEvent {
    RequestReceived(PermissionRequest),
    RequestResponded(String, PermissionResponse), // request_id, response
    PermissionRevoked(String, Vec<Permission>),   // connection_id, permissions
    PermissionExpired(String),                    // connection_id
    SecurityAlert(String),                        // message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub require_permission_for_screen_view: bool,
    pub require_permission_for_input_control: bool,
    pub require_permission_for_file_transfer: bool,
    pub auto_deny_after_minutes: u32,
    pub max_concurrent_connections: usize,
    pub enable_whitelist: bool,
    pub whitelisted_devices: Vec<String>, // connection IDs or device names
    pub default_session_duration_minutes: Option<u32>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            require_permission_for_screen_view: true,
            require_permission_for_input_control: true,
            require_permission_for_file_transfer: true,
            auto_deny_after_minutes: 5,
            max_concurrent_connections: 3,
            enable_whitelist: false,
            whitelisted_devices: Vec::new(),
            default_session_duration_minutes: Some(60), // 1 hour
        }
    }
}

pub struct PermissionManager {
    config: Arc<RwLock<PermissionConfig>>,
    pending_requests: Arc<RwLock<HashMap<String, PermissionRequest>>>,
    active_grants: Arc<RwLock<HashMap<String, PermissionGrant>>>,
    event_sender: Arc<RwLock<Option<mpsc::UnboundedSender<PermissionEvent>>>>,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(PermissionConfig::default())),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            active_grants: Arc::new(RwLock::new(HashMap::new())),
            event_sender: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn initialize(&self) -> Result<mpsc::UnboundedReceiver<PermissionEvent>> {
        info!("Initializing permission manager");
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        {
            let mut sender = self.event_sender.write().await;
            *sender = Some(event_tx);
        }
        
        // Start cleanup task for expired requests and grants
        self.start_cleanup_task().await;
        
        info!("Permission manager initialized");
        Ok(event_rx)
    }
    
    pub async fn request_permission(
        &self,
        connection_id: String,
        device_info: DeviceInfo,
        requested_permissions: Vec<Permission>,
    ) -> Result<String> {
        info!("Permission request from {}: {:?}", device_info.name, requested_permissions);
        
        // Check if device is whitelisted
        let config = self.config.read().await;
        if config.enable_whitelist {
            let is_whitelisted = config.whitelisted_devices.iter().any(|device| {
                device == &connection_id || device == &device_info.name
            });
            
            if is_whitelisted {
                info!("Device {} is whitelisted, auto-granting permissions", device_info.name);
                return self.grant_permission_internal(
                    connection_id,
                    device_info,
                    requested_permissions,
                    config.default_session_duration_minutes,
                ).await;
            }
        }
        
        // Check concurrent connection limit
        let active_grants = self.active_grants.read().await;
        if active_grants.len() >= config.max_concurrent_connections {
            warn!("Too many concurrent connections, denying request from {}", device_info.name);
            return Err(anyhow::anyhow!("Maximum concurrent connections exceeded"));
        }
        drop(active_grants);
        drop(config);
        
        // Create permission request
        let request_id = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::minutes(5); // 5 minute expiry
        
        let request = PermissionRequest {
            id: request_id.clone(),
            connection_id: connection_id.clone(),
            device_name: device_info.name.clone(),
            device_info,
            requested_permissions,
            requested_at: Utc::now(),
            expires_at,
        };
        
        // Store pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id.clone(), request.clone());
        }
        
        // Send event
        if let Some(sender) = self.event_sender.read().await.as_ref() {
            let _ = sender.send(PermissionEvent::RequestReceived(request));
        }
        
        info!("Permission request created with ID: {}", request_id);
        Ok(request_id)
    }
    
    pub async fn respond_to_request(
        &self,
        request_id: String,
        response: PermissionResponse,
    ) -> Result<()> {
        info!("Responding to permission request {}: {:?}", request_id, response);
        
        // Get and remove pending request
        let request = {
            let mut pending = self.pending_requests.write().await;
            pending.remove(&request_id)
                .ok_or_else(|| anyhow::anyhow!("Permission request not found: {}", request_id))?
        };
        
        // Check if request has expired
        if Utc::now() > request.expires_at {
            warn!("Permission request {} has expired", request_id);
            let expired_response = PermissionResponse::Expired;
            
            if let Some(sender) = self.event_sender.read().await.as_ref() {
                let _ = sender.send(PermissionEvent::RequestResponded(request_id, expired_response));
            }
            
            return Ok(());
        }
        
        // Process response
        match &response {
            PermissionResponse::Granted { permissions, duration_minutes } => {
                self.grant_permission_internal(
                    request.connection_id,
                    request.device_info,
                    permissions.clone(),
                    *duration_minutes,
                ).await?;
                
                info!("Granted permissions to {}: {:?}", request.device_name, permissions);
            }
            PermissionResponse::Denied { reason } => {
                info!("Denied permissions to {}: {}", request.device_name, reason);
            }
            PermissionResponse::Expired => {
                info!("Permission request from {} expired", request.device_name);
            }
        }
        
        // Send response event
        if let Some(sender) = self.event_sender.read().await.as_ref() {
            let _ = sender.send(PermissionEvent::RequestResponded(request_id, response));
        }
        
        Ok(())
    }
    
    async fn grant_permission_internal(
        &self,
        connection_id: String,
        device_info: DeviceInfo,
        permissions: Vec<Permission>,
        duration_minutes: Option<u32>,
    ) -> Result<String> {
        let expires_at = duration_minutes.map(|mins| Utc::now() + Duration::minutes(mins as i64));
        
        let grant = PermissionGrant {
            connection_id: connection_id.clone(),
            permissions,
            granted_at: Utc::now(),
            expires_at,
            device_info,
        };
        
        // Store active grant
        {
            let mut active_grants = self.active_grants.write().await;
            active_grants.insert(connection_id.clone(), grant);
        }
        
        Ok(connection_id)
    }
    
    pub async fn check_permission(
        &self,
        connection_id: &str,
        permission: &Permission,
    ) -> bool {
        let active_grants = self.active_grants.read().await;
        
        if let Some(grant) = active_grants.get(connection_id) {
            // Check if grant has expired
            if let Some(expires_at) = grant.expires_at {
                if Utc::now() > expires_at {
                    debug!("Permission grant for {} has expired", connection_id);
                    return false;
                }
            }
            
            // Check if permission is granted
            grant.permissions.contains(permission)
        } else {
            false
        }
    }
    
    pub async fn revoke_permissions(
        &self,
        connection_id: &str,
        permissions: Option<Vec<Permission>>,
    ) -> Result<()> {
        info!("Revoking permissions for connection: {}", connection_id);
        
        let mut active_grants = self.active_grants.write().await;
        
        if let Some(permissions) = permissions {
            // Revoke specific permissions
            if let Some(grant) = active_grants.get_mut(connection_id) {
                grant.permissions.retain(|p| !permissions.contains(p));
                
                // Remove grant if no permissions left
                if grant.permissions.is_empty() {
                    active_grants.remove(connection_id);
                }
                
                // Send revoke event
                if let Some(sender) = self.event_sender.read().await.as_ref() {
                    let _ = sender.send(PermissionEvent::PermissionRevoked(
                        connection_id.to_string(),
                        permissions,
                    ));
                }
            }
        } else {
            // Revoke all permissions
            if let Some(grant) = active_grants.remove(connection_id) {
                // Send revoke event
                if let Some(sender) = self.event_sender.read().await.as_ref() {
                    let _ = sender.send(PermissionEvent::PermissionRevoked(
                        connection_id.to_string(),
                        grant.permissions,
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn get_active_grants(&self) -> Vec<PermissionGrant> {
        let active_grants = self.active_grants.read().await;
        active_grants.values().cloned().collect()
    }
    
    pub async fn get_pending_requests(&self) -> Vec<PermissionRequest> {
        let pending_requests = self.pending_requests.read().await;
        pending_requests.values().cloned().collect()
    }
    
    pub async fn update_config(&self, new_config: PermissionConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated permission configuration");
        Ok(())
    }
    
    pub async fn get_config(&self) -> PermissionConfig {
        self.config.read().await.clone()
    }
    
    async fn start_cleanup_task(&self) {
        let pending_requests = self.pending_requests.clone();
        let active_grants = self.active_grants.clone();
        let event_sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let now = Utc::now();
                
                // Clean up expired requests
                {
                    let mut pending = pending_requests.write().await;
                    let expired_ids: Vec<String> = pending
                        .iter()
                        .filter(|(_, request)| now > request.expires_at)
                        .map(|(id, _)| id.clone())
                        .collect();
                    
                    for id in expired_ids {
                        if let Some(request) = pending.remove(&id) {
                            debug!("Cleaned up expired permission request: {}", request.device_name);
                        }
                    }
                }
                
                // Clean up expired grants
                {
                    let mut active = active_grants.write().await;
                    let expired_connections: Vec<String> = active
                        .iter()
                        .filter(|(_, grant)| {
                            if let Some(expires_at) = grant.expires_at {
                                now > expires_at
                            } else {
                                false
                            }
                        })
                        .map(|(id, _)| id.clone())
                        .collect();
                    
                    for connection_id in expired_connections {
                        if let Some(grant) = active.remove(&connection_id) {
                            debug!("Cleaned up expired permission grant: {}", grant.device_info.name);
                            
                            // Send expiry event
                            if let Some(sender) = event_sender.read().await.as_ref() {
                                let _ = sender.send(PermissionEvent::PermissionExpired(connection_id));
                            }
                        }
                    }
                }
            }
        });
    }
    
    pub async fn get_permission_stats(&self) -> PermissionStats {
        let active_grants = self.active_grants.read().await;
        let pending_requests = self.pending_requests.read().await;
        
        PermissionStats {
            active_connections: active_grants.len(),
            pending_requests: pending_requests.len(),
            total_granted_sessions: 0, // Would track this in persistent storage
            total_denied_requests: 0,  // Would track this in persistent storage
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStats {
    pub active_connections: usize,
    pub pending_requests: usize,
    pub total_granted_sessions: u64,
    pub total_denied_requests: u64,
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}