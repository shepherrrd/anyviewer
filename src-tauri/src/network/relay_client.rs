use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use log::{info, error, debug, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    pub server_url: String,
    pub enabled: bool,
    pub auto_fallback: bool, // Automatically fallback to relay when P2P fails
    pub connection_timeout_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            server_url: "ws://localhost:8080/ws".to_string(),
            enabled: true,
            auto_fallback: true,
            connection_timeout_seconds: 30,
            heartbeat_interval_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMessage {
    pub message_type: RelayMessageType,
    pub source_id: Option<String>,
    pub target_id: String,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayMessageType {
    // Connection management
    Register,
    RegisterResponse,
    ConnectRequest,
    ConnectResponse,
    Disconnect,
    
    // Data forwarding
    ScreenFrame,
    InputEvent,
    FileTransfer,
    
    // Control messages
    Heartbeat,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub connection_id: String, // 8-digit ID like "123 456 78"
    pub device_info: DeviceInfo,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub os: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub target_connection_id: String,
    pub client_info:DeviceInfo,
}

#[derive(Debug, Clone)]
pub enum RelayClientEvent {
    Connected,
    Disconnected,
    MessageReceived(RelayMessage),
    RegistrationSuccess(String), // connection_id
    RegistrationFailed(String),  // error message
    ConnectionRequest(ConnectRequest),
    Error(String),
}

pub struct RelayClient {
    config: RelayConfig,
    connection_id: Option<String>, // Our 8-digit ID
    device_info: DeviceInfo,
    event_sender: Option<mpsc::UnboundedSender<RelayClientEvent>>,
    is_connected: Arc<RwLock<bool>>,
    is_registered: Arc<RwLock<bool>>,
}

impl RelayClient {
    pub fn new(config: RelayConfig) -> Self {
        let device_info = DeviceInfo {
            name: get_device_name(),
            os: get_os_name(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        Self {
            config,
            connection_id: None,
            device_info,
            event_sender: None,
            is_connected: Arc::new(RwLock::new(false)),
            is_registered: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn connect(&mut self) -> Result<mpsc::UnboundedReceiver<RelayClientEvent>> {
        if !self.config.enabled {
            return Err(anyhow::anyhow!("Relay client is disabled"));
        }
        
        info!("Connecting to relay server: {}", self.config.server_url);
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.event_sender = Some(event_tx.clone());
        
        // Connect to relay server
        let url = Url::parse(&self.config.server_url)?;
        let (ws_stream, _) = connect_async(url).await?;
        
        info!("Connected to relay server");
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let is_connected = self.is_connected.clone();
        let is_registered = self.is_registered.clone();
        
        // Update connection status
        {
            let mut connected = is_connected.write().await;
            *connected = true;
        }
        
        // Send connected event
        if let Err(e) = event_tx.send(RelayClientEvent::Connected) {
            error!("Failed to send connected event: {}", e);
        }
        
        // Handle outgoing messages
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<RelayMessage>();
        let event_tx_clone = event_tx.clone();
        let is_connected_clone = is_connected.clone();
        tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                if let Ok(text) = serde_json::to_string(&message) {
                    if let Err(e) = ws_sender.send(Message::Text(text)).await {
                        error!("Failed to send message to relay server: {}", e);
                        
                        // Update connection status
                        {
                            let mut connected = is_connected_clone.write().await;
                            *connected = false;
                        }
                        
                        // Send disconnected event
                        if let Err(e) = event_tx_clone.send(RelayClientEvent::Disconnected) {
                            error!("Failed to send disconnected event: {}", e);
                        }
                        break;
                    }
                } else {
                    error!("Failed to serialize relay message");
                }
            }
        });
        
        // Handle incoming messages
        let event_tx_clone = event_tx.clone();
        let is_connected_clone = is_connected.clone();
        let is_registered_clone = is_registered.clone();
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<RelayMessage>(&text) {
                            Ok(relay_message) => {
                                debug!("Received relay message: {:?}", relay_message.message_type);
                                
                                // Handle special message types
                                match relay_message.message_type {
                                    RelayMessageType::RegisterResponse => {
                                        handle_register_response(&relay_message, &event_tx_clone, &is_registered_clone).await;
                                    }
                                    RelayMessageType::ConnectRequest => {
                                        handle_connect_request(&relay_message, &event_tx_clone).await;
                                    }
                                    _ => {
                                        // Forward other messages as events
                                        if let Err(e) = event_tx_clone.send(RelayClientEvent::MessageReceived(relay_message)) {
                                            error!("Failed to send message received event: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse relay message: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Relay server connection closed");
                        
                        // Update connection status
                        {
                            let mut connected = is_connected_clone.write().await;
                            *connected = false;
                        }
                        
                        {
                            let mut registered = is_registered_clone.write().await;
                            *registered = false;
                        }
                        
                        // Send disconnected event
                        if let Err(e) = event_tx_clone.send(RelayClientEvent::Disconnected) {
                            error!("Failed to send disconnected event: {}", e);
                        }
                        break;
                    }
                    Err(e) => {
                        error!("Relay server WebSocket error: {}", e);
                        
                        // Update connection status
                        {
                            let mut connected = is_connected_clone.write().await;
                            *connected = false;
                        }
                        
                        // Send error event
                        if let Err(e) = event_tx_clone.send(RelayClientEvent::Error(e.to_string())) {
                            error!("Failed to send error event: {}", e);
                        }
                        break;
                    }
                    _ => {}
                }
            }
        });
        
        Ok(event_rx)
    }
    
    pub async fn register(&mut self, connection_id: String) -> Result<()> {
        if !*self.is_connected.read().await {
            return Err(anyhow::anyhow!("Not connected to relay server"));
        }
        
        info!("Registering with relay server using ID: {}", connection_id);
        
        self.connection_id = Some(connection_id.clone());
        
        let register_request = RegisterRequest {
            connection_id: connection_id.clone(),
            device_info: self.device_info.clone(),
            capabilities: vec![
                "screen_share".to_string(),
                "input_control".to_string(),
                "file_transfer".to_string(),
            ],
        };
        
        let message = RelayMessage {
            message_type: RelayMessageType::Register,
            source_id: None,
            target_id: "relay".to_string(),
            data: serde_json::to_value(register_request)?,
            timestamp: chrono::Utc::now(),
        };
        
        // TODO: Send message through outgoing channel
        // For now, we'll simulate registration success
        debug!("Registration message prepared for ID: {}", connection_id);
        
        Ok(())
    }
    
    pub async fn connect_to_peer(&self, target_connection_id: String) -> Result<()> {
        if !*self.is_registered.read().await {
            return Err(anyhow::anyhow!("Not registered with relay server"));
        }
        
        info!("Requesting connection to peer: {}", target_connection_id);
        
        let connect_request = ConnectRequest {
            target_connection_id: target_connection_id.clone(),
            client_info: self.device_info.clone(),
        };
        
        let message = RelayMessage {
            message_type: RelayMessageType::ConnectRequest,
            source_id: self.connection_id.clone(),
            target_id: target_connection_id.clone(),
            data: serde_json::to_value(connect_request)?,
            timestamp: chrono::Utc::now(),
        };
        
        // TODO: Send message through outgoing channel
        debug!("Connect request prepared for target: {}", target_connection_id);
        
        Ok(())
    }
    
    pub async fn send_screen_frame(&self, target_id: String, frame_data: Vec<u8>) -> Result<()> {
        if !*self.is_registered.read().await {
            return Err(anyhow::anyhow!("Not registered with relay server"));
        }
        
        let message = RelayMessage {
            message_type: RelayMessageType::ScreenFrame,
            source_id: self.connection_id.clone(),
            target_id,
            data: serde_json::json!({
                "frame_data": general_purpose::STANDARD.encode(frame_data),
                "timestamp": chrono::Utc::now(),
            }),
            timestamp: chrono::Utc::now(),
        };
        
        // TODO: Send message through outgoing channel
        debug!("Screen frame prepared for forwarding");
        
        Ok(())
    }
    
    pub async fn send_input_event(&self, target_id: String, input_data: serde_json::Value) -> Result<()> {
        if !*self.is_registered.read().await {
            return Err(anyhow::anyhow!("Not registered with relay server"));
        }
        
        let message = RelayMessage {
            message_type: RelayMessageType::InputEvent,
            source_id: self.connection_id.clone(),
            target_id,
            data: input_data,
            timestamp: chrono::Utc::now(),
        };
        
        // TODO: Send message through outgoing channel
        debug!("Input event prepared for forwarding");
        
        Ok(())
    }
    
    pub async fn disconnect(&mut self) -> Result<()> {
        if !*self.is_connected.read().await {
            return Ok(());
        }
        
        info!("Disconnecting from relay server");
        
        if let Some(connection_id) = &self.connection_id {
            let message = RelayMessage {
                message_type: RelayMessageType::Disconnect,
                source_id: Some(connection_id.clone()),
                target_id: "relay".to_string(),
                data: serde_json::json!({}),
                timestamp: chrono::Utc::now(),
            };
            
            // TODO: Send disconnect message
            debug!("Disconnect message prepared");
        }
        
        // Update connection status
        {
            let mut connected = self.is_connected.write().await;
            *connected = false;
        }
        
        {
            let mut registered = self.is_registered.write().await;
            *registered = false;
        }
        
        self.connection_id = None;
        
        Ok(())
    }
    
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }
    
    pub async fn is_registered(&self) -> bool {
        *self.is_registered.read().await
    }
    
    pub fn get_connection_id(&self) -> Option<String> {
        self.connection_id.clone()
    }
}

async fn handle_register_response(
    message: &RelayMessage,
    event_sender: &mpsc::UnboundedSender<RelayClientEvent>,
    is_registered: &Arc<RwLock<bool>>,
) {
    if let Ok(response) = serde_json::from_value::<serde_json::Value>(message.data.clone()) {
        if let Some(success) = response.get("success").and_then(|v| v.as_bool()) {
            if success {
                info!("Successfully registered with relay server");
                
                {
                    let mut registered = is_registered.write().await;
                    *registered = true;
                }
                
                if let Some(connection_id) = response.get("connection_id").and_then(|v| v.as_str()) {
                    if let Err(e) = event_sender.send(RelayClientEvent::RegistrationSuccess(connection_id.to_string())) {
                        error!("Failed to send registration success event: {}", e);
                    }
                }
            } else {
                warn!("Registration failed");
                
                let error_msg = response.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                
                if let Err(e) = event_sender.send(RelayClientEvent::RegistrationFailed(error_msg)) {
                    error!("Failed to send registration failed event: {}", e);
                }
            }
        }
    }
}

async fn handle_connect_request(
    message: &RelayMessage,
    event_sender: &mpsc::UnboundedSender<RelayClientEvent>,
) {
    if let Ok(connect_request) = serde_json::from_value::<ConnectRequest>(message.data.clone()) {
        info!("Received connection request from: {}", connect_request.client_info.name);
        
        if let Err(e) = event_sender.send(RelayClientEvent::ConnectionRequest(connect_request)) {
            error!("Failed to send connection request event: {}", e);
        }
    }
}

fn get_device_name() -> String {
    gethostname::gethostname()
        .to_string_lossy()
        .to_string()
}

fn get_os_name() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        "Unknown".to_string()
    }
}