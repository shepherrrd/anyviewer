pub mod server;
pub mod client;
pub mod protocol;
pub mod p2p;
pub mod relay_client;
pub mod connection_manager;
pub mod discovery;
pub mod connection_requests;

use anyhow::Result;
use log::{info, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

pub use discovery::*;
pub use connection_requests::{IncomingConnectionRequest, ConnectionRequestResponse, ConnectionRequestManager};

pub use server::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResponse {
    pub success: bool,
    pub session_id: String,
    pub server_info: Option<ServerInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub version: String,
    pub capabilities: Vec<String>,
    pub encryption_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub server_port: u16,
    pub max_connections: usize,
    pub enable_encryption: bool,
    pub relay_server_url: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_port: 7878,
            max_connections: 10,
            enable_encryption: true,
            relay_server_url: None,
        }
    }
}

pub struct NetworkManager {
    config: Arc<RwLock<NetworkConfig>>,
    active_sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    server: Option<Arc<RemoteDesktopServer>>,
    discovery: Option<Arc<NetworkDiscovery>>,
    device_updates_rx: Option<mpsc::UnboundedReceiver<Vec<DiscoveredDevice>>>,
    connection_requests: Option<Arc<ConnectionRequestManager>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub client_address: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub is_host: bool,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(NetworkConfig::default())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            server: None,
            discovery: None,
            device_updates_rx: None,
            connection_requests: None,
        }
    }
    
    pub async fn start_host_server(&self) -> Result<String> {
        let config = self.config.read().await;
        let port = config.server_port;
        drop(config);
        
        info!("Starting host server on port {}", port);
        
        let server = RemoteDesktopServer::new(port).await?;
        let session_id = Uuid::new_v4().to_string();
        
        // Store session info
        let session_info = SessionInfo {
            id: session_id.clone(),
            client_address: "0.0.0.0".to_string(),
            connected_at: chrono::Utc::now(),
            is_host: true,
        };
        
        self.active_sessions.write().await.insert(session_id.clone(), session_info);
        
        // Start server in background
        let active_sessions = self.active_sessions.clone();
        let server_arc = Arc::new(server);
        
        tokio::spawn(async move {
            if let Err(e) = server_arc.start().await {
                error!("Server error: {}", e);
            }
        });
        
        info!("Host server started with session ID: {}", session_id);
        Ok(session_id)
    }
    
    pub async fn connect_to_host(&self, request: ConnectionRequest) -> Result<ConnectionResponse> {
        info!("Attempting to connect to session: {}", request.session_id);
        
        // In a real implementation, this would connect to a relay server
        // or attempt direct connection to discover the host
        
        // For now, simulate a successful connection
        let server_info = ServerInfo {
            version: "1.0.0".to_string(),
            capabilities: vec![
                "screen_capture".to_string(),
                "input_forwarding".to_string(),
                "file_transfer".to_string(),
            ],
            encryption_enabled: true,
        };
        
        let session_info = SessionInfo {
            id: request.session_id.clone(),
            client_address: "remote".to_string(),
            connected_at: chrono::Utc::now(),
            is_host: false,
        };
        
        self.active_sessions.write().await.insert(request.session_id.clone(), session_info);
        
        Ok(ConnectionResponse {
            success: true,
            session_id: request.session_id,
            server_info: Some(server_info),
            error: None,
        })
    }
    
    pub async fn disconnect_session(&self, session_id: &str) -> Result<()> {
        info!("Disconnecting session: {}", session_id);
        
        self.active_sessions.write().await.remove(session_id);
        
        // Additional cleanup would go here
        
        Ok(())
    }
    
    pub async fn get_active_sessions(&self) -> Vec<SessionInfo> {
        self.active_sessions.read().await.values().cloned().collect()
    }
    
    pub async fn update_config(&self, new_config: NetworkConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated network configuration");
        Ok(())
    }
    
    pub async fn start_discovery(&mut self, device_name: String) -> Result<mpsc::UnboundedReceiver<Vec<DiscoveredDevice>>> {
        if self.discovery.is_some() {
            return Err(anyhow::anyhow!("Discovery already started"));
        }

        let config = self.config.read().await;
        let server_port = config.server_port;
        drop(config);

        let (device_updates_tx, device_updates_rx) = mpsc::unbounded_channel();
        
        let discovery = Arc::new(NetworkDiscovery::new(
            device_name,
            server_port,
            device_updates_tx,
        ));

        discovery.start().await?;
        self.discovery = Some(discovery);

        info!("Network discovery started");
        Ok(device_updates_rx)
    }

    pub async fn stop_discovery(&mut self) -> Result<()> {
        if let Some(discovery) = &self.discovery {
            discovery.stop().await?;
            self.discovery = None;
            info!("Network discovery stopped");
        }
        Ok(())
    }

    pub async fn get_discovered_devices(&self) -> Vec<DiscoveredDevice> {
        if let Some(discovery) = &self.discovery {
            discovery.get_discovered_devices().await
        } else {
            Vec::new()
        }
    }

    pub async fn connect_to_discovered_device(&self, device_id: &str) -> Result<ConnectionResponse> {
        let devices = self.get_discovered_devices().await;
        
        if let Some(device) = devices.iter().find(|d| d.info.device_id == device_id) {
            // Create connection request to the discovered device
            let request = ConnectionRequest {
                session_id: Uuid::new_v4().to_string(),
            };

            // In a real implementation, we would connect to the device's server port
            // For now, we'll simulate the connection
            info!("Connecting to device: {} at {}:{}", 
                  device.info.device_name, 
                  device.info.ip_address, 
                  device.info.server_port);

            let server_info = ServerInfo {
                version: device.info.version.clone(),
                capabilities: device.info.capabilities.clone(),
                encryption_enabled: true,
            };

            let session_info = SessionInfo {
                id: request.session_id.clone(),
                client_address: device.address.to_string(),
                connected_at: chrono::Utc::now(),
                is_host: false,
            };

            self.active_sessions.write().await.insert(request.session_id.clone(), session_info);

            Ok(ConnectionResponse {
                success: true,
                session_id: request.session_id,
                server_info: Some(server_info),
                error: None,
            })
        } else {
            Err(anyhow::anyhow!("Device not found: {}", device_id))
        }
    }

    pub async fn initialize_connection_requests(&mut self) -> Result<(mpsc::UnboundedReceiver<IncomingConnectionRequest>, mpsc::UnboundedReceiver<ConnectionRequestResponse>)> {
        if self.connection_requests.is_some() {
            return Err(anyhow::anyhow!("Connection requests already initialized"));
        }

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        let manager = Arc::new(ConnectionRequestManager::new(request_tx, response_tx));
        manager.start().await?;
        
        self.connection_requests = Some(manager);
        
        info!("Connection request manager initialized");
        Ok((request_rx, response_rx))
    }

    pub async fn create_connection_request(
        &self,
        requester_device_id: String,
        requester_name: String,
        requester_ip: String,
        requested_permissions: Vec<String>,
        message: Option<String>,
    ) -> Result<String> {
        if let Some(manager) = &self.connection_requests {
            manager
                .create_connection_request(
                    requester_device_id,
                    requester_name,
                    requester_ip,
                    requested_permissions,
                    message,
                )
                .await
        } else {
            Err(anyhow::anyhow!("Connection request manager not initialized"))
        }
    }

    pub async fn respond_to_connection_request(
        &self,
        request_id: String,
        accepted: bool,
        granted_permissions: Vec<String>,
        session_duration_minutes: Option<u32>,
        denial_reason: Option<String>,
    ) -> Result<()> {
        if let Some(manager) = &self.connection_requests {
            manager
                .respond_to_request(
                    request_id,
                    accepted,
                    granted_permissions,
                    session_duration_minutes,
                    denial_reason,
                )
                .await
        } else {
            Err(anyhow::anyhow!("Connection request manager not initialized"))
        }
    }

    pub async fn get_pending_connection_requests(&self) -> Vec<IncomingConnectionRequest> {
        if let Some(manager) = &self.connection_requests {
            manager.get_pending_requests().await
        } else {
            Vec::new()
        }
    }

    pub async fn cancel_connection_request(&self, request_id: &str) -> Result<()> {
        if let Some(manager) = &self.connection_requests {
            manager.cancel_request(request_id).await
        } else {
            Err(anyhow::anyhow!("Connection request manager not initialized"))
        }
    }

    pub async fn get_network_stats(&self) -> NetworkStats {
        let sessions = self.get_active_sessions().await;
        
        NetworkStats {
            active_connections: sessions.len(),
            total_sessions: sessions.len(), // In a real app, this would be persistent
            bytes_sent: 0, // Would be tracked in real implementation
            bytes_received: 0,
            uptime: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    pub active_connections: usize,
    pub total_sessions: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub uptime: u64,
}