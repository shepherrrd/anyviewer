use anyhow::Result;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, sleep};
use uuid::Uuid;

const DISCOVERY_PORT: u16 = 7879;
const BROADCAST_INTERVAL: Duration = Duration::from_secs(5);
const DEVICE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub message_type: MessageType,
    pub device_info: DeviceInfo,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Announce,
    Response,
    Goodbye,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub server_port: u16,
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub info: DeviceInfo,
    pub last_seen: SystemTime,
    pub address: SocketAddr,
}

pub struct NetworkDiscovery {
    device_info: DeviceInfo,
    discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
    is_running: Arc<RwLock<bool>>,
    device_updates_tx: mpsc::UnboundedSender<Vec<DiscoveredDevice>>,
}

impl NetworkDiscovery {
    pub fn new(
        device_name: String,
        server_port: u16,
        device_updates_tx: mpsc::UnboundedSender<Vec<DiscoveredDevice>>,
    ) -> Self {
        let device_info = DeviceInfo {
            device_id: Uuid::new_v4().to_string(),
            device_name,
            device_type: "AnyViewer".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![
                "screen_capture".to_string(),
                "input_forwarding".to_string(),
                "file_transfer".to_string(),
            ],
            server_port,
            ip_address: Self::get_local_ip().unwrap_or_else(|_| "127.0.0.1".to_string()),
        };

        Self {
            device_info,
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            device_updates_tx,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        info!("Starting network discovery service on port {}", DISCOVERY_PORT);

        // Start UDP listener
        let discovered_devices = self.discovered_devices.clone();
        let device_info = self.device_info.clone();
        let is_running_clone = self.is_running.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::run_udp_listener(discovered_devices, device_info, is_running_clone).await {
                error!("UDP listener error: {}", e);
            }
        });

        // Start periodic announcements
        let device_info = self.device_info.clone();
        let is_running_clone = self.is_running.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::run_announcements(device_info, is_running_clone).await {
                error!("Announcements error: {}", e);
            }
        });

        // Start cleanup task
        let discovered_devices = self.discovered_devices.clone();
        let is_running_clone = self.is_running.clone();
        let device_updates_tx = self.device_updates_tx.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::run_cleanup_task(discovered_devices, is_running_clone, device_updates_tx).await {
                error!("Cleanup task error: {}", e);
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        *is_running = false;

        info!("Stopping network discovery service");

        // Send goodbye message
        let goodbye_message = DiscoveryMessage {
            message_type: MessageType::Goodbye,
            device_info: self.device_info.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        if let Err(e) = Self::send_broadcast(&goodbye_message).await {
            warn!("Failed to send goodbye message: {}", e);
        }

        self.discovered_devices.write().await.clear();
        Ok(())
    }

    pub async fn get_discovered_devices(&self) -> Vec<DiscoveredDevice> {
        self.discovered_devices.read().await.values().cloned().collect()
    }

    async fn run_udp_listener(
        discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
        device_info: DeviceInfo,
        is_running: Arc<RwLock<bool>>,
    ) -> Result<()> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;

        let mut buffer = [0u8; 1024];

        while *is_running.read().await {
            match socket.recv_from(&mut buffer) {
                Ok((size, addr)) => {
                    if let Ok(message_str) = std::str::from_utf8(&buffer[..size]) {
                        if let Ok(message) = serde_json::from_str::<DiscoveryMessage>(message_str) {
                            // Don't process our own messages
                            if message.device_info.device_id == device_info.device_id {
                                continue;
                            }

                            match message.message_type {
                                MessageType::Announce => {
                                    // Respond to announcement
                                    let response = DiscoveryMessage {
                                        message_type: MessageType::Response,
                                        device_info: device_info.clone(),
                                        timestamp: SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs(),
                                    };

                                    if let Err(e) = Self::send_to_address(&response, addr).await {
                                        warn!("Failed to send response to {}: {}", addr, e);
                                    }

                                    // Add to discovered devices
                                    Self::add_discovered_device(&discovered_devices, message, addr).await;
                                }
                                MessageType::Response => {
                                    // Add to discovered devices
                                    Self::add_discovered_device(&discovered_devices, message, addr).await;
                                }
                                MessageType::Goodbye => {
                                    // Remove from discovered devices
                                    discovered_devices.write().await.remove(&message.device_info.device_id);
                                    info!("Device {} said goodbye", message.device_info.device_name);
                                }
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, continue
                    sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    warn!("UDP receive error: {}", e);
                    sleep(Duration::from_millis(1000)).await;
                }
            }
        }

        Ok(())
    }

    async fn run_announcements(
        device_info: DeviceInfo,
        is_running: Arc<RwLock<bool>>,
    ) -> Result<()> {
        let mut announce_interval = interval(BROADCAST_INTERVAL);

        while *is_running.read().await {
            announce_interval.tick().await;

            let announce_message = DiscoveryMessage {
                message_type: MessageType::Announce,
                device_info: device_info.clone(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };

            if let Err(e) = Self::send_broadcast(&announce_message).await {
                warn!("Failed to send announcement: {}", e);
            }
        }

        Ok(())
    }

    async fn run_cleanup_task(
        discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
        is_running: Arc<RwLock<bool>>,
        device_updates_tx: mpsc::UnboundedSender<Vec<DiscoveredDevice>>,
    ) -> Result<()> {
        let mut cleanup_interval = interval(Duration::from_secs(10));

        while *is_running.read().await {
            cleanup_interval.tick().await;

            let now = SystemTime::now();
            let mut devices = discovered_devices.write().await;
            let before_count = devices.len();

            devices.retain(|_, device| {
                now.duration_since(device.last_seen).unwrap_or_default() < DEVICE_TIMEOUT
            });

            let after_count = devices.len();
            if before_count != after_count {
                info!("Cleaned up {} stale devices", before_count - after_count);
                
                // Send updated device list
                let current_devices: Vec<DiscoveredDevice> = devices.values().cloned().collect();
                drop(devices);
                
                if device_updates_tx.send(current_devices).is_err() {
                    warn!("Failed to send device updates");
                }
            }
        }

        Ok(())
    }

    async fn add_discovered_device(
        discovered_devices: &Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
        message: DiscoveryMessage,
        addr: SocketAddr,
    ) {
        let device = DiscoveredDevice {
            info: message.device_info.clone(),
            last_seen: SystemTime::now(),
            address: addr,
        };

        let mut devices = discovered_devices.write().await;
        let is_new = !devices.contains_key(&message.device_info.device_id);
        devices.insert(message.device_info.device_id.clone(), device);

        if is_new {
            info!("Discovered new device: {} at {}", message.device_info.device_name, addr);
        }
    }

    async fn send_broadcast(message: &DiscoveryMessage) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;

        let data = serde_json::to_string(message)?;
        let broadcast_addr = format!("255.255.255.255:{}", DISCOVERY_PORT);
        
        socket.send_to(data.as_bytes(), broadcast_addr)?;
        Ok(())
    }

    async fn send_to_address(message: &DiscoveryMessage, addr: SocketAddr) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let data = serde_json::to_string(message)?;
        
        socket.send_to(data.as_bytes(), addr)?;
        Ok(())
    }

    fn get_local_ip() -> Result<String> {
        // Get the first non-loopback IP address
        use std::net::{IpAddr, Ipv4Addr};
        
        match local_ip_address::local_ip() {
            Ok(IpAddr::V4(ip)) if ip != Ipv4Addr::LOCALHOST => Ok(ip.to_string()),
            _ => Ok("127.0.0.1".to_string()),
        }
    }
}