use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingConnectionRequest {
    pub request_id: String,
    pub requester_device_id: String,
    pub requester_name: String,
    pub requester_ip: String,
    pub requested_permissions: Vec<String>,
    pub message: Option<String>,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequestResponse {
    pub request_id: String,
    pub accepted: bool,
    pub granted_permissions: Vec<String>,
    pub session_duration_minutes: Option<u32>,
    pub denial_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingRequest {
    pub request: IncomingConnectionRequest,
    pub expires_at: SystemTime,
}

pub struct ConnectionRequestManager {
    pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>,
    request_updates_tx: mpsc::UnboundedSender<IncomingConnectionRequest>,
    response_updates_tx: mpsc::UnboundedSender<ConnectionRequestResponse>,
    request_timeout: Duration,
}

impl ConnectionRequestManager {
    pub fn new(
        request_updates_tx: mpsc::UnboundedSender<IncomingConnectionRequest>,
        response_updates_tx: mpsc::UnboundedSender<ConnectionRequestResponse>,
    ) -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            request_updates_tx,
            response_updates_tx,
            request_timeout: Duration::from_secs(60), // 1 minute timeout
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting connection request manager");

        // Start cleanup task for expired requests
        let pending_requests = self.pending_requests.clone();
        tokio::spawn(async move {
            Self::run_cleanup_task(pending_requests).await;
        });

        Ok(())
    }

    pub async fn create_connection_request(
        &self,
        requester_device_id: String,
        requester_name: String,
        requester_ip: String,
        requested_permissions: Vec<String>,
        message: Option<String>,
    ) -> Result<String> {
        let request_id = Uuid::new_v4().to_string();
        let now = SystemTime::now();

        let request = IncomingConnectionRequest {
            request_id: request_id.clone(),
            requester_device_id,
            requester_name,
            requester_ip,
            requested_permissions,
            message,
            timestamp: now,
        };

        let pending_request = PendingRequest {
            request: request.clone(),
            expires_at: now + self.request_timeout,
        };

        let requester_name = pending_request.request.requester_name.clone();
        
        // Store the pending request
        self.pending_requests.write().await.insert(request_id.clone(), pending_request);

        // Notify UI about new request
        if let Err(e) = self.request_updates_tx.send(request) {
            warn!("Failed to send request update: {}", e);
        }

        info!("Created connection request {} for device {}", request_id, requester_name);
        Ok(request_id)
    }

    pub async fn respond_to_request(
        &self,
        request_id: String,
        accepted: bool,
        granted_permissions: Vec<String>,
        session_duration_minutes: Option<u32>,
        denial_reason: Option<String>,
    ) -> Result<()> {
        // Remove from pending requests
        let request = self.pending_requests.write().await.remove(&request_id);

        if request.is_none() {
            return Err(anyhow::anyhow!("Request not found or expired: {}", request_id));
        }

        let response = ConnectionRequestResponse {
            request_id: request_id.clone(),
            accepted,
            granted_permissions,
            session_duration_minutes,
            denial_reason,
        };

        // Notify about response
        if let Err(e) = self.response_updates_tx.send(response) {
            warn!("Failed to send response update: {}", e);
        }

        if accepted {
            info!("Connection request {} accepted", request_id);
        } else {
            info!("Connection request {} denied", request_id);
        }

        Ok(())
    }

    pub async fn get_pending_requests(&self) -> Vec<IncomingConnectionRequest> {
        self.pending_requests
            .read()
            .await
            .values()
            .map(|pending| pending.request.clone())
            .collect()
    }

    pub async fn cancel_request(&self, request_id: &str) -> Result<()> {
        let removed = self.pending_requests.write().await.remove(request_id);
        
        if removed.is_some() {
            info!("Cancelled connection request: {}", request_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Request not found: {}", request_id))
        }
    }

    pub async fn handle_incoming_discovery_request(
        &self,
        request_data: crate::network::discovery::ConnectionRequestData,
    ) -> Result<()> {
        let now = SystemTime::now();

        let request = IncomingConnectionRequest {
            request_id: request_data.request_id.clone(),
            requester_device_id: request_data.requester_device_id,
            requester_name: request_data.requester_name.clone(),
            requester_ip: request_data.requester_ip,
            requested_permissions: request_data.requested_permissions,
            message: request_data.message,
            timestamp: now,
        };

        let pending_request = PendingRequest {
            request: request.clone(),
            expires_at: now + self.request_timeout,
        };

        // Store the pending request
        self.pending_requests.write().await.insert(request_data.request_id.clone(), pending_request);

        // Notify UI about new request
        if let Err(e) = self.request_updates_tx.send(request) {
            warn!("Failed to send request update: {}", e);
        }

        info!("Handled incoming connection request {} from {}", request_data.request_id, request_data.requester_name);
        Ok(())
    }

    async fn run_cleanup_task(pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            let now = SystemTime::now();
            let mut requests = pending_requests.write().await;
            let before_count = requests.len();

            requests.retain(|_, pending| now < pending.expires_at);

            let after_count = requests.len();
            if before_count != after_count {
                info!("Cleaned up {} expired connection requests", before_count - after_count);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionRequestStats {
    pub pending_requests: usize,
    pub total_requests_today: usize,
    pub accepted_requests_today: usize,
    pub denied_requests_today: usize,
}

impl ConnectionRequestManager {
    pub async fn get_stats(&self) -> ConnectionRequestStats {
        let pending_count = self.pending_requests.read().await.len();
        
        // In a real implementation, these would be tracked persistently
        ConnectionRequestStats {
            pending_requests: pending_count,
            total_requests_today: 0,
            accepted_requests_today: 0,
            denied_requests_today: 0,
        }
    }
}