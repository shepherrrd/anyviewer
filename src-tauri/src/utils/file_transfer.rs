use anyhow::Result;
use log::{info, debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{mpsc, RwLock, Semaphore};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub transfer_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub chunk_size: usize,
    pub compression: CompressionType,
    pub encryption_enabled: bool,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferResponse {
    pub transfer_id: String,
    pub accepted: bool,
    pub reason: Option<String>,
    pub suggested_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub transfer_id: String,
    pub chunk_index: u64,
    pub data: Vec<u8>,
    pub is_compressed: bool,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferProgress {
    pub transfer_id: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Lz4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    Transferring,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TransferConfig {
    pub max_concurrent_transfers: usize,
    pub chunk_size: usize,
    pub max_speed_bps: Option<u64>, // Rate limiting
    pub enable_compression: bool,
    pub compression_level: u32,
    pub max_file_size: u64,
    pub allowed_extensions: Option<Vec<String>>,
    pub download_directory: PathBuf,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 3,
            chunk_size: 64 * 1024, // 64KB chunks for optimal performance
            max_speed_bps: None,
            enable_compression: true,
            compression_level: 6,
            max_file_size: 10 * 1024 * 1024 * 1024, // 10GB
            allowed_extensions: None, // Allow all extensions
            download_directory: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

pub struct FileTransferManager {
    config: Arc<RwLock<TransferConfig>>,
    active_transfers: Arc<RwLock<HashMap<String, TransferSession>>>,
    transfer_semaphore: Arc<Semaphore>,
    event_sender: mpsc::UnboundedSender<TransferEvent>,
}

#[derive(Debug, Clone)]
struct TransferSession {
    pub id: String,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub bytes_transferred: u64,
    pub start_time: Instant,
    pub last_chunk_time: Instant,
    pub status: TransferStatus,
    pub is_upload: bool,
    pub speed_samples: Vec<(Instant, u64)>,
}

#[derive(Debug, Clone)]
pub enum TransferEvent {
    TransferStarted(String, String), // transfer_id, file_name
    ProgressUpdate(FileTransferProgress),
    TransferCompleted(String),
    TransferFailed(String, String), // transfer_id, error
    TransferCancelled(String),
}

impl FileTransferManager {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TransferEvent>) {
        let config = TransferConfig::default();
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let manager = Self {
            config: Arc::new(RwLock::new(config.clone())),
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            transfer_semaphore: Arc::new(Semaphore::new(config.max_concurrent_transfers)),
            event_sender,
        };
        
        (manager, event_receiver)
    }
    
    /// Start sending a file
    pub async fn send_file(&self, file_path: &Path) -> Result<String> {
        // Validate file
        if !file_path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", file_path.display()));
        }
        
        let metadata = fs::metadata(file_path).await?;
        if !metadata.is_file() {
            return Err(anyhow::anyhow!("Path is not a file: {}", file_path.display()));
        }
        
        let config = self.config.read().await;
        
        // Check file size limit
        if metadata.len() > config.max_file_size {
            return Err(anyhow::anyhow!("File too large: {} bytes (max: {} bytes)", 
                                     metadata.len(), config.max_file_size));
        }
        
        // Check file extension if restricted
        if let Some(ref allowed_exts) = config.allowed_extensions {
            if let Some(ext) = file_path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if !allowed_exts.contains(&ext_str) {
                    return Err(anyhow::anyhow!("File extension not allowed: {}", ext_str));
                }
            }
        }
        
        let transfer_id = Uuid::new_v4().to_string();
        let file_name = file_path.file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
            .to_string_lossy()
            .to_string();
        
        // Calculate checksum
        let checksum = self.calculate_file_checksum(file_path).await?;
        
        // Create transfer request
        let request = FileTransferRequest {
            transfer_id: transfer_id.clone(),
            file_name: file_name.clone(),
            file_size: metadata.len(),
            chunk_size: config.chunk_size,
            compression: if config.enable_compression { 
                CompressionType::Lz4 
            } else { 
                CompressionType::None 
            },
            encryption_enabled: true,
            checksum: Some(checksum),
        };
        
        // Create transfer session
        let session = TransferSession {
            id: transfer_id.clone(),
            file_path: file_path.to_path_buf(),
            file_size: metadata.len(),
            bytes_transferred: 0,
            start_time: Instant::now(),
            last_chunk_time: Instant::now(),
            status: TransferStatus::Pending,
            is_upload: true,
            speed_samples: Vec::new(),
        };
        
        self.active_transfers.write().await.insert(transfer_id.clone(), session);
        
        info!("File transfer initiated: {} ({} bytes)", file_name, metadata.len());
        
        // Send transfer started event
        let _ = self.event_sender.send(TransferEvent::TransferStarted(
            transfer_id.clone(), 
            file_name
        ));
        
        Ok(transfer_id)
    }
    
    /// Accept an incoming file transfer
    pub async fn accept_file_transfer(&self, request: FileTransferRequest, save_path: Option<PathBuf>) -> Result<FileTransferResponse> {
        let config = self.config.read().await;
        
        // Determine save path
        let file_path = if let Some(path) = save_path {
            path
        } else {
            config.download_directory.join(&request.file_name)
        };
        
        // Check if file already exists
        if file_path.exists() {
            return Ok(FileTransferResponse {
                transfer_id: request.transfer_id,
                accepted: false,
                reason: Some("File already exists".to_string()),
                suggested_path: Some(self.generate_unique_path(&file_path).await?),
            });
        }
        
        // Check available space (simplified)
        // In a real implementation, you'd check disk space
        
        // Create transfer session
        let session = TransferSession {
            id: request.transfer_id.clone(),
            file_path: file_path.clone(),
            file_size: request.file_size,
            bytes_transferred: 0,
            start_time: Instant::now(),
            last_chunk_time: Instant::now(),
            status: TransferStatus::Pending,
            is_upload: false,
            speed_samples: Vec::new(),
        };
        
        self.active_transfers.write().await.insert(request.transfer_id.clone(), session);
        
        info!("Accepting file transfer: {} -> {}", request.file_name, file_path.display());
        
        Ok(FileTransferResponse {
            transfer_id: request.transfer_id,
            accepted: true,
            reason: None,
            suggested_path: Some(file_path.to_string_lossy().to_string()),
        })
    }
    
    /// Send file chunk (for upload)
    pub async fn send_chunk(&self, transfer_id: &str, chunk_index: u64) -> Result<FileChunk> {
        let permit = self.transfer_semaphore.acquire().await?;
        
        let mut transfers = self.active_transfers.write().await;
        let session = transfers.get_mut(transfer_id)
            .ok_or_else(|| anyhow::anyhow!("Transfer not found: {}", transfer_id))?;
        
        if !session.is_upload {
            return Err(anyhow::anyhow!("Not an upload transfer"));
        }
        
        let config = self.config.read().await;
        let chunk_size = config.chunk_size;
        let enable_compression = config.enable_compression;
        drop(config);
        
        // Calculate chunk offset
        let offset = chunk_index * chunk_size as u64;
        if offset >= session.file_size {
            return Err(anyhow::anyhow!("Chunk index out of bounds"));
        }
        
        // Read chunk from file
        let mut file = File::open(&session.file_path)?;
        file.seek(SeekFrom::Start(offset))?;
        
        let actual_chunk_size = ((session.file_size - offset) as usize).min(chunk_size);
        let mut buffer = vec![0u8; actual_chunk_size];
        file.read_exact(&mut buffer)?;
        
        // Compress if enabled
        let (data, is_compressed) = if enable_compression {
            match self.compress_data(&buffer) {
                Ok(compressed) => {
                    if compressed.len() < buffer.len() {
                        (compressed, true)
                    } else {
                        (buffer, false)
                    }
                },
                Err(_) => (buffer, false),
            }
        } else {
            (buffer, false)
        };
        
        // Calculate checksum
        let checksum = self.calculate_data_checksum(&data);
        
        // Update session
        session.bytes_transferred += actual_chunk_size as u64;
        session.last_chunk_time = Instant::now();
        session.status = TransferStatus::Transferring;
        
        // Update speed tracking
        self.update_speed_tracking(session).await;
        
        // Send progress update
        let progress = self.calculate_progress(session);
        let _ = self.event_sender.send(TransferEvent::ProgressUpdate(progress));
        
        drop(transfers);
        drop(permit);
        
        debug!("Sent chunk {} for transfer {} ({} bytes, compressed: {})", 
               chunk_index, transfer_id, data.len(), is_compressed);
        
        Ok(FileChunk {
            transfer_id: transfer_id.to_string(),
            chunk_index,
            data,
            is_compressed,
            checksum,
        })
    }
    
    /// Receive file chunk (for download)
    pub async fn receive_chunk(&self, chunk: FileChunk) -> Result<()> {
        let permit = self.transfer_semaphore.acquire().await?;
        
        let mut transfers = self.active_transfers.write().await;
        let session = transfers.get_mut(&chunk.transfer_id)
            .ok_or_else(|| anyhow::anyhow!("Transfer not found: {}", chunk.transfer_id))?;
        
        if session.is_upload {
            return Err(anyhow::anyhow!("Not a download transfer"));
        }
        
        // Verify checksum
        let calculated_checksum = self.calculate_data_checksum(&chunk.data);
        if calculated_checksum != chunk.checksum {
            return Err(anyhow::anyhow!("Chunk checksum mismatch"));
        }
        
        // Decompress if needed
        let data = if chunk.is_compressed {
            self.decompress_data(&chunk.data)?
        } else {
            chunk.data
        };
        
        // Write chunk to file
        let chunk_size = data.len() as u64;
        let offset = chunk.chunk_index * chunk_size;
        
        // Ensure parent directory exists
        if let Some(parent) = session.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Open file for writing (create if not exists)
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&session.file_path)?;
        
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&data)?;
        file.flush()?;
        
        // Update session
        session.bytes_transferred += data.len() as u64;
        session.last_chunk_time = Instant::now();
        session.status = TransferStatus::Transferring;
        
        // Update speed tracking
        self.update_speed_tracking(session).await;
        
        // Check if transfer is complete
        if session.bytes_transferred >= session.file_size {
            session.status = TransferStatus::Completed;
            let _ = self.event_sender.send(TransferEvent::TransferCompleted(chunk.transfer_id.clone()));
            info!("File transfer completed: {}", session.file_path.display());
        } else {
            let progress = self.calculate_progress(session);
            let _ = self.event_sender.send(TransferEvent::ProgressUpdate(progress));
        }
        
        drop(transfers);
        drop(permit);
        
        debug!("Received chunk {} for transfer {} ({} bytes)", 
               chunk.chunk_index, chunk.transfer_id, data.len());
        
        Ok(())
    }
    
    /// Cancel a transfer
    pub async fn cancel_transfer(&self, transfer_id: &str) -> Result<()> {
        let mut transfers = self.active_transfers.write().await;
        
        if let Some(mut session) = transfers.remove(transfer_id) {
            session.status = TransferStatus::Cancelled;
            
            // If it's a download, optionally remove partial file
            if !session.is_upload && session.bytes_transferred < session.file_size {
                if let Err(e) = fs::remove_file(&session.file_path).await {
                    warn!("Failed to remove partial file: {}", e);
                }
            }
            
            let _ = self.event_sender.send(TransferEvent::TransferCancelled(transfer_id.to_string()));
            info!("Transfer cancelled: {}", transfer_id);
        }
        
        Ok(())
    }
    
    /// Get transfer progress
    pub async fn get_transfer_progress(&self, transfer_id: &str) -> Option<FileTransferProgress> {
        let transfers = self.active_transfers.read().await;
        transfers.get(transfer_id).map(|session| self.calculate_progress(session))
    }
    
    /// Get all active transfers
    pub async fn get_active_transfers(&self) -> Vec<FileTransferProgress> {
        let transfers = self.active_transfers.read().await;
        transfers.values().map(|session| self.calculate_progress(session)).collect()
    }
    
    // Helper methods
    
    async fn calculate_file_checksum(&self, file_path: &Path) -> Result<String> {
        let data = fs::read(file_path).await?;
        Ok(self.calculate_data_checksum(&data))
    }
    
    fn calculate_data_checksum(&self, data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
    
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Simple compression using flate2 (would be better with lz4)
        use std::io::Write;
        use flate2::Compression;
        use flate2::write::GzEncoder;
        
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }
    
    fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        use std::io::Read;
        use flate2::read::GzDecoder;
        
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }
    
    async fn generate_unique_path(&self, base_path: &Path) -> Result<String> {
        let mut counter = 1;
        let stem = base_path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = base_path.extension().unwrap_or_default().to_string_lossy();
        let parent = base_path.parent().unwrap_or(Path::new("."));
        
        loop {
            let new_name = if extension.is_empty() {
                format!("{} ({})", stem, counter)
            } else {
                format!("{} ({}).{}", stem, counter, extension)
            };
            
            let new_path = parent.join(new_name);
            if !new_path.exists() {
                return Ok(new_path.to_string_lossy().to_string());
            }
            
            counter += 1;
            if counter > 1000 {
                return Err(anyhow::anyhow!("Too many duplicate files"));
            }
        }
    }
    
    async fn update_speed_tracking(&self, session: &mut TransferSession) {
        let now = Instant::now();
        session.speed_samples.push((now, session.bytes_transferred));
        
        // Keep only recent samples (last 10 seconds)
        session.speed_samples.retain(|(time, _)| now.duration_since(*time) < Duration::from_secs(10));
    }
    
    fn calculate_progress(&self, session: &TransferSession) -> FileTransferProgress {
        // Calculate speed
        let speed_bps = if session.speed_samples.len() >= 2 {
            let (oldest_time, oldest_bytes) = session.speed_samples[0];
            let (newest_time, newest_bytes) = session.speed_samples[session.speed_samples.len() - 1];
            
            let time_diff = newest_time.duration_since(oldest_time).as_secs_f64();
            if time_diff > 0.0 {
                ((newest_bytes - oldest_bytes) as f64 / time_diff) as u64
            } else {
                0
            }
        } else {
            0
        };
        
        // Calculate ETA
        let eta_seconds = if speed_bps > 0 && session.bytes_transferred < session.file_size {
            Some((session.file_size - session.bytes_transferred) / speed_bps)
        } else {
            None
        };
        
        FileTransferProgress {
            transfer_id: session.id.clone(),
            bytes_transferred: session.bytes_transferred,
            total_bytes: session.file_size,
            speed_bps,
            eta_seconds,
            status: session.status.clone(),
        }
    }
}

// Add flate2 dependency to Cargo.toml for compression
// flate2 = "1.0"