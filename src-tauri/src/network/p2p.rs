use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use log::{info, error, debug, warn};
use serde_json;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

use crate::utils::id_generator::{IdGenerator, ConnectionId};
use super::protocol::{ProtocolMessage, MessageType};

pub struct P2PManager {
    id_generator: Arc<IdGenerator>,
    active_connections: Arc<RwLock<HashMap<String, P2PConnection>>>,
    connection_listeners: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2PEvent>>>>,
    is_host: Arc<RwLock<bool>>,
    current_connection_id: Arc<RwLock<Option<ConnectionId>>>, 
}

#[derive(Debug, Clone)]
pub struct P2PConnection {
    pub connection_id: String,
    pub peer_address: SocketAddr,
    pub is_authenticated: bool,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_ping: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub enum P2PEvent {
    ConnectionEstablished(String, SocketAddr),
    ConnectionLost(String),
    MessageReceived(String, ProtocolMessage),
    AuthenticationSuccess(String),
    AuthenticationFailed(String, String),
    HostStarted(ConnectionId),
    HostStopped,
}

#[derive(Debug, Clone)]
pub enum P2PConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

impl P2PManager {
    pub fn new() -> Self {
        Self {
            id_generator: Arc::new(IdGenerator::new()),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_listeners: Arc::new(RwLock::new(HashMap::new())),
            is_host: Arc::new(RwLock::new(false)),
            current_connection_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Start hosting with P2P capability - generates 8-digit ID
    pub async fn start_host(&self, port: u16) -> Result<ConnectionId> {
        info!("Starting P2P host on port {}", port);
        
        // Generate unique 8-digit connection ID
        let connection_id = self.id_generator.generate_connection_id()?;
        let session_id = Uuid::new_v4().to_string();
        
        // Register the session
        self.id_generator.register_session(&connection_id, session_id.clone())?;
        
        // Set as host
        *self.is_host.write().await = true;
        *self.current_connection_id.write().await = Some(connection_id.clone());
        
        // Start TCP listener for P2P connections
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await?;
        info!("P2P host listening on {} with ID: {}", addr, connection_id.formatted_id);
        
        // Clone necessary data for the spawn
        let active_connections = self.active_connections.clone();
        let connection_listeners = self.connection_listeners.clone();
        let id_generator = self.id_generator.clone();
        let connection_id_clone = connection_id.clone();
        
        // Spawn connection acceptor
        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                info!("New P2P connection from {}", addr);
                
                let active_connections = active_connections.clone();
                let connection_listeners = connection_listeners.clone();
                let id_generator = id_generator.clone();
                let connection_id = connection_id_clone.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = Self::handle_p2p_connection(
                        stream, 
                        addr, 
                        active_connections, 
                        connection_listeners,
                        id_generator,
                        connection_id,
                        true // is_host
                    ).await {
                        error!("P2P connection error: {}", e);
                    }
                });
            }
        });
        
        // Notify listeners
        self.notify_event(P2PEvent::HostStarted(connection_id.clone())).await;
        
        Ok(connection_id)
    }
    
    /// Connect to a host using 8-digit ID (P2P on same network)
    pub async fn connect_to_host(&self, formatted_id: &str, host_ip: Option<String>) -> Result<String> {
        info!("Connecting to host with ID: {}", formatted_id);
        
        // Validate ID format
        if !self.id_generator.validate_id_format(formatted_id) {
            return Err(anyhow::anyhow!("Invalid connection ID format"));
        }
        
        // If no host IP provided, try to discover on local network
        let host_address = if let Some(ip) = host_ip {
            format!("{}:7878", ip) // Default port
        } else {
            // Try to discover host on local network
            self.discover_host_on_network(formatted_id).await?
        };
        
        info!("Attempting to connect to {}", host_address);
        
        // Connect to host
        let stream = TcpStream::connect(&host_address).await?;
        let peer_addr = stream.peer_addr()?;
        
        let active_connections = self.active_connections.clone();
        let connection_listeners = self.connection_listeners.clone();
        let id_generator = self.id_generator.clone();
        
        // Parse the connection ID
        let numeric_id = self.id_generator.parse_connection_id(formatted_id)?;
        let connection_id = ConnectionId {
            id: numeric_id.to_string(),
            numeric_id,
            formatted_id: formatted_id.to_string(),
        };
        
        // Handle connection
        let connection_uuid = Uuid::new_v4().to_string();
        tokio::spawn(async move {
            if let Err(e) = Self::handle_p2p_connection(
                stream,
                peer_addr,
                active_connections,
                connection_listeners,
                id_generator,
                connection_id,
                false // is_host
            ).await {
                error!("P2P client connection error: {}", e);
            }
        });
        
        Ok(connection_uuid)
    }
    
    /// Discover host on local network using broadcast
    async fn discover_host_on_network(&self, formatted_id: &str) -> Result<String> {
        debug!("Discovering host {} on local network", formatted_id);
        
        // Create UDP socket for discovery
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        
        // Create discovery message
        let discovery_msg = serde_json::json!({
            "type": "discovery",
            "connection_id": formatted_id,
            "timestamp": chrono::Utc::now()
        });
        
        let discovery_data = serde_json::to_vec(&discovery_msg)?;
        
        // Broadcast to common subnets
        let broadcast_addresses = vec![
            "192.168.1.255:7879",
            "192.168.0.255:7879", 
            "10.0.0.255:7879",
            "172.16.255.255:7879",
        ];
        
        for addr in &broadcast_addresses {
            if let Err(e) = socket.send_to(&discovery_data, addr) {
                debug!("Failed to broadcast to {}: {}", addr, e);
            }
        }
        
        // Listen for responses (with timeout)
        let mut buffer = [0u8; 1024];
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        
        match socket.recv_from(&mut buffer) {
            Ok((size, addr)) => {
                let response: serde_json::Value = serde_json::from_slice(&buffer[..size])?;
                if response["type"] == "discovery_response" && response["connection_id"] == formatted_id {
                    let host_ip = addr.ip().to_string();
                    info!("Discovered host at {}", host_ip);
                    return Ok(format!("{}:7878", host_ip));
                }
            },
            Err(e) => {
                warn!("No discovery response received: {}", e);
            }
        }
        
        Err(anyhow::anyhow!("Could not discover host on local network"))
    }
    
    /// Handle P2P WebSocket connection
    async fn handle_p2p_connection(
        stream: TcpStream,
        addr: SocketAddr,
        active_connections: Arc<RwLock<HashMap<String, P2PConnection>>>,
        connection_listeners: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2PEvent>>>>,
        id_generator: Arc<IdGenerator>,
        connection_id: ConnectionId,
        is_host: bool,
    ) -> Result<()> {
        let ws_stream = accept_async(stream).await?;
        let connection_uuid = Uuid::new_v4().to_string();
        
        // Register connection
        let connection_info = P2PConnection {
            connection_id: connection_id.formatted_id.clone(),
            peer_address: addr,
            is_authenticated: false,
            connected_at: chrono::Utc::now(),
            last_ping: None,
        };
        
        active_connections.write().await.insert(connection_uuid.clone(), connection_info);
        
        // Notify connection established
        let listeners = connection_listeners.read().await;
        for sender in listeners.values() {
            let _ = sender.send(P2PEvent::ConnectionEstablished(connection_uuid.clone(), addr));
        }
        drop(listeners);
        
        // Handle WebSocket messages
        let result = Self::handle_websocket_messages(
            ws_stream,
            connection_uuid.clone(),
            active_connections.clone(),
            connection_listeners.clone(),
            is_host,
        ).await;
        
        // Cleanup on disconnect
        active_connections.write().await.remove(&connection_uuid);
        
        let listeners = connection_listeners.read().await;
        for sender in listeners.values() {
            let _ = sender.send(P2PEvent::ConnectionLost(connection_uuid.clone()));
        }
        
        result
    }
    
    /// Handle WebSocket message exchange
    async fn handle_websocket_messages(
        mut ws_stream: WebSocketStream<TcpStream>,
        connection_id: String,
        active_connections: Arc<RwLock<HashMap<String, P2PConnection>>>,
        connection_listeners: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2PEvent>>>>,
        is_host: bool,
    ) -> Result<()> {
        while let Some(msg) = ws_stream.next().await {
            match msg? {
                Message::Text(text) => {
                    debug!("Received P2P message: {}", text);
                    
                    if let Ok(protocol_msg) = serde_json::from_str::<ProtocolMessage>(&text) {
                        // Update last ping time
                        if protocol_msg.message_type == MessageType::Heartbeat {
                            let mut connections = active_connections.write().await;
                            if let Some(conn) = connections.get_mut(&connection_id) {
                                conn.last_ping = Some(chrono::Utc::now());
                            }
                        }
                        
                        // Notify listeners
                        let listeners = connection_listeners.read().await;
                        for sender in listeners.values() {
                            let _ = sender.send(P2PEvent::MessageReceived(
                                connection_id.clone(), 
                                protocol_msg.clone()
                            ));
                        }
                    }
                }
                Message::Binary(data) => {
                    debug!("Received P2P binary data: {} bytes", data.len());
                    // Handle binary data (screen frames, file transfers, etc.)
                }
                Message::Ping(payload) => {
                    ws_stream.send(Message::Pong(payload)).await?;
                }
                Message::Pong(_) => {
                    // Update ping time
                    let mut connections = active_connections.write().await;
                    if let Some(conn) = connections.get_mut(&connection_id) {
                        conn.last_ping = Some(chrono::Utc::now());
                    }
                }
                Message::Close(_) => {
                    info!("P2P connection {} closed", connection_id);
                    break;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// Stop hosting
    pub async fn stop_host(&self) -> Result<()> {
        info!("Stopping P2P host");
        
        *self.is_host.write().await = false;
        
        // Release connection ID
        if let Some(connection_id) = self.current_connection_id.write().await.take() {
            self.id_generator.release_id(&connection_id)?;
        }
        
        // Close all connections
        self.active_connections.write().await.clear();
        
        // Notify listeners
        self.notify_event(P2PEvent::HostStopped).await;
        
        Ok(())
    }
    
    /// Register event listener
    pub async fn register_event_listener(&self, listener_id: String) -> mpsc::UnboundedReceiver<P2PEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.connection_listeners.write().await.insert(listener_id, tx);
        rx
    }
    
    /// Remove event listener
    pub async fn remove_event_listener(&self, listener_id: &str) {
        self.connection_listeners.write().await.remove(listener_id);
    }
    
    /// Notify all event listeners
    async fn notify_event(&self, event: P2PEvent) {
        let listeners = self.connection_listeners.read().await;
        for sender in listeners.values() {
            let _ = sender.send(event.clone());
        }
    }
    
    /// Get active connections
    pub async fn get_active_connections(&self) -> Vec<P2PConnection> {
        self.active_connections.read().await.values().cloned().collect()
    }
    
    /// Check if currently hosting
    pub async fn is_hosting(&self) -> bool {
        *self.is_host.read().await
    }
    
    /// Get current connection ID if hosting
    pub async fn get_current_connection_id(&self) -> Option<ConnectionId> {
        self.current_connection_id.read().await.clone()
    }

    /// Start discovery for peers
    pub async fn start_discovery(&self) -> Result<()> {
        info!("Starting P2P discovery");
        // P2P discovery is typically handled by the connection attempts
        // For now, this is a no-op but could be expanded to include mDNS discovery
        Ok(())
    }

    /// Connect to a peer using connection ID
    pub async fn connect_to_peer(&self, connection_id: ConnectionId) -> Result<P2PConnectionStatus> {
        info!("Connecting to peer: {}", connection_id.formatted_id);
        
        // For now, delegate to connect_to_host with extracted ID
        match self.connect_to_host(&connection_id.formatted_id, None).await {
            Ok(_) => Ok(P2PConnectionStatus::Connected),
            Err(_) => Ok(P2PConnectionStatus::Failed),
        }
    }

    /// Disconnect from current session
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting P2P session");
        
        // Clear current connection
        *self.current_connection_id.write().await = None;
        *self.is_host.write().await = false;
        
        // Clear active connections
        self.active_connections.write().await.clear();
        
        Ok(())
    }

    /// Get list of discovered peers
    pub async fn get_discovered_peers(&self) -> Result<Vec<ConnectionId>> {
        // For now, return active connections as "discovered peers"
        let connections = self.active_connections.read().await;
        let peers = connections
            .values()
            .map(|conn| ConnectionId {
                id: conn.connection_id.clone(),
                numeric_id: 0, // Would need to be properly parsed
                formatted_id: conn.connection_id.clone(),
            })
            .collect();
        
        Ok(peers)
    }
}