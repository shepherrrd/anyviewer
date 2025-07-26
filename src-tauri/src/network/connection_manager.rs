use anyhow::Result;
use log::{info, error, debug, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

use crate::network::p2p::{P2PManager, P2PConnectionStatus};
use crate::network::relay_client::{RelayClient, RelayConfig, RelayClientEvent};
use crate::utils::id_generator::{IdGenerator, ConnectionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub p2p_enabled: bool,
    pub relay_enabled: bool,
    pub auto_fallback_to_relay: bool,
    pub connection_timeout_seconds: u64,
    pub relay_config: RelayConfig,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            p2p_enabled: true,
            relay_enabled: true,
            auto_fallback_to_relay: true,
            connection_timeout_seconds: 30,
            relay_config: RelayConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    P2P,
    Relay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(ConnectionType),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    StatusChanged(ConnectionStatus),
    ConnectionRequest {
        from_id: String,
        device_name: String,
        requires_permission: bool,
    },
    DataReceived {
        from_id: String,
        data_type: String,
        data: Vec<u8>,
    },
    Error(String),
}

pub struct ConnectionManager {
    config: Arc<RwLock<ConnectionConfig>>,
    id_generator: Arc<IdGenerator>,
    p2p_manager: Arc<RwLock<Option<P2PManager>>>,
    relay_client: Arc<RwLock<Option<RelayClient>>>,
    current_connection_id: Arc<RwLock<Option<ConnectionId>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    event_sender: Arc<RwLock<Option<mpsc::UnboundedSender<ConnectionEvent>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ConnectionConfig::default())),
            id_generator: Arc::new(IdGenerator::new()),
            p2p_manager: Arc::new(RwLock::new(None)),
            relay_client: Arc::new(RwLock::new(None)),
            current_connection_id: Arc::new(RwLock::new(None)),
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            event_sender: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn initialize(&self) -> Result<mpsc::UnboundedReceiver<ConnectionEvent>> {
        info!("Initializing connection manager");
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        {
            let mut sender = self.event_sender.write().await;
            *sender = Some(event_tx.clone());
        }
        
        // Generate connection ID
        let connection_id = self.id_generator.generate_connection_id()?;
        {
            let mut current_id = self.current_connection_id.write().await;
            *current_id = Some(connection_id.clone());
        }
        
        info!("Generated connection ID: {}", connection_id.formatted_id);
        
        // Initialize P2P manager if enabled
        let config = self.config.read().await;
        if config.p2p_enabled {
            let mut p2p_manager = P2PManager::new();
            p2p_manager.start_discovery().await?;
            
            let mut p2p_manager_lock = self.p2p_manager.write().await;
            *p2p_manager_lock = Some(p2p_manager);
            
            info!("P2P manager initialized");
        }
        
        // Initialize relay client if enabled
        if config.relay_enabled {
            let relay_client = RelayClient::new(config.relay_config.clone());
            
            let mut relay_client_lock = self.relay_client.write().await;
            *relay_client_lock = Some(relay_client);
            
            info!("Relay client initialized");
        }
        
        Ok(event_rx)
    }
    
    pub async fn start_hosting(&self) -> Result<String> {
        info!("Starting hosting mode");
        
        // Update status
        self.update_status(ConnectionStatus::Connecting).await;
        
        let connection_id = {
            let current_id = self.current_connection_id.read().await;
            current_id.as_ref().ok_or_else(|| anyhow::anyhow!("No connection ID generated"))?.clone()
        };
        
        let config = self.config.read().await;
        let mut connection_established = false;
        
        // Try P2P first if enabled
        if config.p2p_enabled {
            if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
                match p2p_manager.start_host(8080).await {
                    Ok(_) => {
                        info!("P2P hosting started successfully");
                        self.update_status(ConnectionStatus::Connected(ConnectionType::P2P)).await;
                        connection_established = true;
                    }
                    Err(e) => {
                        warn!("P2P hosting failed: {}", e);
                        
                        if !config.auto_fallback_to_relay {
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        // Fallback to relay if P2P failed and relay is enabled
        if !connection_established && config.relay_enabled {
            if let Some(relay_client) = self.relay_client.write().await.as_mut() {
                // Connect to relay server
                let mut relay_events = relay_client.connect().await?;
                
                // Register with relay server
                relay_client.register(connection_id.formatted_id.clone()).await?;
                
                // Handle relay events in background
                let event_sender = self.event_sender.clone();
                let connection_status = self.connection_status.clone();
                tokio::spawn(async move {
                    while let Some(event) = relay_events.recv().await {
                        match event {
                            RelayClientEvent::Connected => {
                                info!("Connected to relay server");
                            }
                            RelayClientEvent::RegistrationSuccess(_) => {
                                info!("Successfully registered with relay server");
                                {
                                    let mut status = connection_status.write().await;
                                    *status = ConnectionStatus::Connected(ConnectionType::Relay);
                                }
                                
                                if let Some(sender) = event_sender.read().await.as_ref() {
                                    let _ = sender.send(ConnectionEvent::StatusChanged(
                                        ConnectionStatus::Connected(ConnectionType::Relay)
                                    ));
                                }
                            }
                            RelayClientEvent::ConnectionRequest(request) => {
                                info!("Received connection request via relay: {}", request.client_info.name);
                                
                                if let Some(sender) = event_sender.read().await.as_ref() {
                                    let _ = sender.send(ConnectionEvent::ConnectionRequest {
                                        from_id: request.target_connection_id,
                                        device_name: request.client_info.name,
                                        requires_permission: true,
                                    });
                                }
                            }
                            RelayClientEvent::Error(error) => {
                                error!("Relay client error: {}", error);
                                
                                if let Some(sender) = event_sender.read().await.as_ref() {
                                    let _ = sender.send(ConnectionEvent::Error(error));
                                }
                            }
                            _ => {}
                        }
                    }
                });
                
                info!("Relay hosting started successfully");
                connection_established = true;
            }
        }
        
        if !connection_established {
            let error_msg = "Failed to establish any connection (P2P and Relay)";
            self.update_status(ConnectionStatus::Failed(error_msg.to_string())).await;
            return Err(anyhow::anyhow!(error_msg));
        }
        
        Ok(connection_id.formatted_id)
    }
    
    pub async fn connect_to_host(&self, target_connection_id: String) -> Result<()> {
        info!("Attempting to connect to host: {}", target_connection_id);
        
        // Update status
        self.update_status(ConnectionStatus::Connecting).await;
        
        let config = self.config.read().await;
        let mut connection_established = false;
        
        // Try P2P first if enabled
        if config.p2p_enabled {
            if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
                // Create ConnectionId from string
                let connection_id = ConnectionId {
                    id: target_connection_id.clone(),
                    numeric_id: target_connection_id.replace(" ", "").parse::<u32>().unwrap_or(0),
                    formatted_id: target_connection_id.clone(),
                };
                match p2p_manager.connect_to_peer(connection_id).await {
                    Ok(status) => {
                        match status {
                            P2PConnectionStatus::Connected => {
                                info!("P2P connection established");
                                self.update_status(ConnectionStatus::Connected(ConnectionType::P2P)).await;
                                connection_established = true;
                            }
                            P2PConnectionStatus::Failed => {
                                warn!("P2P connection failed");
                                
                                if !config.auto_fallback_to_relay {
                                    return Err(anyhow::anyhow!("P2P connection failed"));
                                }
                            }
                            _ => {
                                debug!("P2P connection in progress");
                            }
                        }
                    }
                    Err(e) => {
                        warn!("P2P connection error: {}", e);
                        
                        if !config.auto_fallback_to_relay {
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        // Fallback to relay if P2P failed and relay is enabled
        if !connection_established && config.relay_enabled {
            if let Some(relay_client) = self.relay_client.write().await.as_mut() {
                // Connect to relay server if not already connected
                if !relay_client.is_connected().await {
                    let _relay_events = relay_client.connect().await?;
                    
                    let connection_id = {
                        let current_id = self.current_connection_id.read().await;
                        current_id.as_ref().ok_or_else(|| anyhow::anyhow!("No connection ID generated"))?.clone()
                    };
                    
                    relay_client.register(connection_id.formatted_id).await?;
                }
                
                // Request connection to target
                relay_client.connect_to_peer(target_connection_id).await?;
                
                info!("Relay connection request sent");
                self.update_status(ConnectionStatus::Connected(ConnectionType::Relay)).await;
                connection_established = true;
            }
        }
        
        if !connection_established {
            let error_msg = "Failed to establish connection via P2P or Relay";
            self.update_status(ConnectionStatus::Failed(error_msg.to_string())).await;
            return Err(anyhow::anyhow!(error_msg));
        }
        
        Ok(())
    }
    
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from all connections");
        
        // Disconnect P2P
        if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
            if let Err(e) = p2p_manager.disconnect().await {
                error!("Failed to disconnect P2P: {}", e);
            }
        }
        
        // Disconnect relay
        if let Some(relay_client) = self.relay_client.write().await.as_mut() {
            if let Err(e) = relay_client.disconnect().await {
                error!("Failed to disconnect relay: {}", e);
            }
        }
        
        // Update status
        self.update_status(ConnectionStatus::Disconnected).await;
        
        Ok(())
    }
    
    pub async fn send_screen_frame(&self, frame_data: Vec<u8>) -> Result<()> {
        let status = self.connection_status.read().await;
        
        match status.clone() {
            ConnectionStatus::Connected(ConnectionType::P2P) => {
                if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
                    // P2P screen frame sending would be implemented here
                    debug!("Sending screen frame via P2P ({} bytes)", frame_data.len());
                }
            }
            ConnectionStatus::Connected(ConnectionType::Relay) => {
                if let Some(relay_client) = self.relay_client.read().await.as_ref() {
                    // Get target ID from current connection context
                    // This would be tracked in a real implementation
                    let target_id = "target_connection_id".to_string(); // Placeholder
                    relay_client.send_screen_frame(target_id, frame_data).await?;
                }
            }
            _ => {
                return Err(anyhow::anyhow!("No active connection to send screen frame"));
            }
        }
        
        Ok(())
    }
    
    pub async fn send_input_event(&self, input_data: serde_json::Value) -> Result<()> {
        let status = self.connection_status.read().await;
        
        match status.clone() {
            ConnectionStatus::Connected(ConnectionType::P2P) => {
                if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
                    // P2P input event sending would be implemented here
                    debug!("Sending input event via P2P");
                }
            }
            ConnectionStatus::Connected(ConnectionType::Relay) => {
                if let Some(relay_client) = self.relay_client.read().await.as_ref() {
                    // Get target ID from current connection context
                    let target_id = "target_connection_id".to_string(); // Placeholder
                    relay_client.send_input_event(target_id, input_data).await?;
                }
            }
            _ => {
                return Err(anyhow::anyhow!("No active connection to send input event"));
            }
        }
        
        Ok(())
    }
    
    pub async fn get_connection_status(&self) -> ConnectionStatus {
        self.connection_status.read().await.clone()
    }
    
    pub async fn get_connection_id(&self) -> Option<String> {
        self.current_connection_id.read().await.as_ref().map(|id| id.formatted_id.clone())
    }
    
    pub async fn update_config(&self, new_config: ConnectionConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated connection configuration");
        Ok(())
    }
    
    pub async fn get_available_peers(&self) -> Vec<String> {
        let mut peers = Vec::new();
        
        // Get P2P discovered peers
        if let Some(p2p_manager) = self.p2p_manager.read().await.as_ref() {
            if let Ok(p2p_peers) = p2p_manager.get_discovered_peers().await {
                peers.extend(p2p_peers.into_iter().map(|p| p.formatted_id));
            }
        }
        
        // Note: Relay peers would typically be discovered through the relay server
        // or through a directory service
        
        peers
    }
    
    async fn update_status(&self, new_status: ConnectionStatus) {
        {
            let mut status = self.connection_status.write().await;
            *status = new_status.clone();
        }
        
        // Send status change event
        if let Some(sender) = self.event_sender.read().await.as_ref() {
            let _ = sender.send(ConnectionEvent::StatusChanged(new_status));
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}