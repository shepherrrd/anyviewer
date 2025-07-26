pub mod screen_streamer;
pub mod compression;
pub mod frame_buffer;

use anyhow::Result;
use log::{info, error};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Duration, Instant};

pub use screen_streamer::*;
pub use frame_buffer::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub target_fps: u32,
    pub quality: u8, // 1-100, higher = better quality
    pub compression_type: CompressionType,
    pub adaptive_quality: bool,
    pub max_bandwidth_mbps: f32,
    pub enable_delta_compression: bool,
    pub buffer_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            quality: 75,
            compression_type: CompressionType::JPEG,
            adaptive_quality: true,
            max_bandwidth_mbps: 10.0,
            enable_delta_compression: true,
            buffer_size: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    JPEG,
    WebP,
    H264,
    VP8,
    AV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingStats {
    pub fps: f32,
    pub bandwidth_mbps: f32,
    pub average_frame_size_kb: f32,
    pub compression_ratio: f32,
    pub latency_ms: f32,
    pub dropped_frames: u64,
    pub total_frames: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone)]
pub enum StreamingEvent {
    FrameReady(Vec<u8>),
    QualityAdjusted(u8),
    FrameDropped(String), // reason
    Error(String),
    StatUpdate(StreamingStats),
}

pub struct StreamingManager {
    config: Arc<RwLock<StreamingConfig>>,
    screen_streamer: Arc<RwLock<Option<ScreenStreamer>>>,
    frame_buffer: Arc<FrameBuffer>,
    stats: Arc<RwLock<StreamingStats>>,
    event_sender: Arc<RwLock<Option<mpsc::UnboundedSender<StreamingEvent>>>>,
    is_streaming: Arc<RwLock<bool>>,
    start_time: Arc<RwLock<Option<Instant>>>,
}

impl StreamingManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(StreamingConfig::default())),
            screen_streamer: Arc::new(RwLock::new(None)),
            frame_buffer: Arc::new(FrameBuffer::new(3)),
            stats: Arc::new(RwLock::new(StreamingStats {
                fps: 0.0,
                bandwidth_mbps: 0.0,
                average_frame_size_kb: 0.0,
                compression_ratio: 0.0,
                latency_ms: 0.0,
                dropped_frames: 0,
                total_frames: 0,
                uptime_seconds: 0,
            })),
            event_sender: Arc::new(RwLock::new(None)),
            is_streaming: Arc::new(RwLock::new(false)),
            start_time: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn initialize(&self) -> Result<mpsc::UnboundedReceiver<StreamingEvent>> {
        info!("Initializing streaming manager");
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        {
            let mut sender = self.event_sender.write().await;
            *sender = Some(event_tx);
        }
        
        // Initialize screen streamer
        let config = self.config.read().await;
        let screen_streamer = ScreenStreamer::new(config.clone()).await?;
        
        {
            let mut streamer = self.screen_streamer.write().await;
            *streamer = Some(screen_streamer);
        }
        
        info!("Streaming manager initialized");
        Ok(event_rx)
    }
    
    pub async fn start_streaming(&self) -> Result<()> {
        info!("Starting screen streaming");
        
        // Set streaming flag
        {
            let mut streaming = self.is_streaming.write().await;
            *streaming = true;
        }
        
        // Set start time
        {
            let mut start_time = self.start_time.write().await;
            *start_time = Some(Instant::now());
        }
        
        // Start streaming loop
        let screen_streamer = self.screen_streamer.clone();
        let frame_buffer = self.frame_buffer.clone();
        let config = self.config.clone();
        let is_streaming = self.is_streaming.clone();
        let event_sender = self.event_sender.clone();
        let stats = self.stats.clone();
        let start_time = self.start_time.clone();
        
        tokio::spawn(async move {
            let mut frame_interval = {
                let config = config.read().await;
                interval(Duration::from_millis(1000 / config.target_fps as u64))
            };
            
            let mut frame_count = 0u64;
            let mut total_frame_size = 0u64;
            let mut last_stats_update = Instant::now();
            
            while *is_streaming.read().await {
                frame_interval.tick().await;
                
                let frame_start = Instant::now();
                
                // Capture and compress frame
                if let Some(streamer) = screen_streamer.read().await.as_ref() {
                    match streamer.capture_and_compress().await {
                        Ok(compressed_frame) => {
                            let frame_size = compressed_frame.len();
                            total_frame_size += frame_size as u64;
                            frame_count += 1;
                            
                            // Add to buffer
                            frame_buffer.add_frame(compressed_frame.clone()).await;
                            
                            // Send frame event
                            if let Some(sender) = event_sender.read().await.as_ref() {
                                let _ = sender.send(StreamingEvent::FrameReady(compressed_frame));
                            }
                            
                            // Update stats periodically
                            if last_stats_update.elapsed() >= Duration::from_secs(1) {
                                let uptime = if let Some(start) = start_time.read().await.as_ref() {
                                    start.elapsed().as_secs()
                                } else {
                                    0
                                };
                                
                                let fps = frame_count as f32 / last_stats_update.elapsed().as_secs_f32();
                                let avg_frame_size = if frame_count > 0 {
                                    (total_frame_size as f32 / frame_count as f32) / 1024.0
                                } else {
                                    0.0
                                };
                                let bandwidth = (fps * avg_frame_size * 8.0) / 1000.0; // Mbps
                                let latency = frame_start.elapsed().as_millis() as f32;
                                
                                let new_stats = StreamingStats {
                                    fps,
                                    bandwidth_mbps: bandwidth,
                                    average_frame_size_kb: avg_frame_size,
                                    compression_ratio: 0.0, // Would calculate from raw vs compressed
                                    latency_ms: latency,
                                    dropped_frames: 0, // Track this separately
                                    total_frames: frame_count,
                                    uptime_seconds: uptime,
                                };
                                
                                {
                                    let mut stats_lock = stats.write().await;
                                    *stats_lock = new_stats.clone();
                                }
                                
                                if let Some(sender) = event_sender.read().await.as_ref() {
                                    let _ = sender.send(StreamingEvent::StatUpdate(new_stats));
                                }
                                
                                last_stats_update = Instant::now();
                                frame_count = 0;
                                total_frame_size = 0;
                            }
                        }
                        Err(e) => {
                            error!("Frame capture error: {}", e);
                            
                            if let Some(sender) = event_sender.read().await.as_ref() {
                                let _ = sender.send(StreamingEvent::Error(e.to_string()));
                            }
                        }
                    }
                }
            }
            
            info!("Streaming loop ended");
        });
        
        info!("Screen streaming started");
        Ok(())
    }
    
    pub async fn stop_streaming(&self) -> Result<()> {
        info!("Stopping screen streaming");
        
        {
            let mut streaming = self.is_streaming.write().await;
            *streaming = false;
        }
        
        {
            let mut start_time = self.start_time.write().await;
            *start_time = None;
        }
        
        Ok(())
    }
    
    pub async fn is_streaming(&self) -> bool {
        *self.is_streaming.read().await
    }
    
    pub async fn get_stats(&self) -> StreamingStats {
        self.stats.read().await.clone()
    }
    
    pub async fn update_config(&self, new_config: StreamingConfig) -> Result<()> {
        info!("Updating streaming configuration");
        
        {
            let mut config = self.config.write().await;
            *config = new_config.clone();
        }
        
        // Update screen streamer if it exists
        if let Some(streamer) = self.screen_streamer.write().await.as_mut() {
            streamer.update_config(new_config).await?;
        }
        
        Ok(())
    }
    
    pub async fn adjust_quality(&self, new_quality: u8) -> Result<()> {
        if new_quality == 0 || new_quality > 100 {
            return Err(anyhow::anyhow!("Quality must be between 1 and 100"));
        }
        
        {
            let mut config = self.config.write().await;
            config.quality = new_quality;
        }
        
        if let Some(streamer) = self.screen_streamer.write().await.as_mut() {
            streamer.set_quality(new_quality).await?;
        }
        
        // Send quality adjustment event
        if let Some(sender) = self.event_sender.read().await.as_ref() {
            let _ = sender.send(StreamingEvent::QualityAdjusted(new_quality));
        }
        
        info!("Adjusted streaming quality to {}", new_quality);
        Ok(())
    }
    
    pub async fn get_latest_frame(&self) -> Option<Vec<u8>> {
        self.frame_buffer.get_latest_frame().await
    }
    
    pub async fn get_frame_buffer_size(&self) -> usize {
        self.frame_buffer.get_buffer_size().await
    }
}

impl Default for StreamingManager {
    fn default() -> Self {
        Self::new()
    }
}