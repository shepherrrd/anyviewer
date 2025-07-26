use anyhow::Result;
use image::{ImageBuffer, RgbaImage};
use log::{info, debug};
use screenshots::Screen;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

use super::StreamingConfig;
use super::compression::Compressor;
use crate::capture::ScreenCaptureManager;

pub struct ScreenStreamer {
    config: StreamingConfig,
    capture_manager: ScreenCaptureManager,
    compressor: Arc<RwLock<Compressor>>,
    last_frame: Arc<RwLock<Option<RgbaImage>>>,
    frame_counter: Arc<RwLock<u64>>,
}

impl ScreenStreamer {
    pub async fn new(config: StreamingConfig) -> Result<Self> {
        info!("Creating screen streamer with config: {:?}", config);
        
        let capture_manager = ScreenCaptureManager::new()?;
        let compressor = Arc::new(RwLock::new(Compressor::new(config.clone())?));
        
        Ok(Self {
            config,
            capture_manager,
            compressor,
            last_frame: Arc::new(RwLock::new(None)),
            frame_counter: Arc::new(RwLock::new(0)),
        })
    }
    
    pub async fn capture_and_compress(&self) -> Result<Vec<u8>> {
        let start_time = Instant::now();
        
        // Capture screen
        let screen_data = self.capture_manager.capture_primary_screen().await?;
        let capture_time = start_time.elapsed();
        
        // Convert to image
        let image = self.bytes_to_image(screen_data)?;
        let conversion_time = start_time.elapsed() - capture_time;
        
        // Check if we should use delta compression
        let should_use_delta = self.config.enable_delta_compression;
        let compressed_data = if should_use_delta {
            if let Some(last_frame) = self.last_frame.read().await.as_ref() {
                // Compress only the differences
                self.compress_frame_delta(&image, last_frame).await?
            } else {
                // First frame, compress fully
                self.compress_frame_full(&image).await?
            }
        } else {
            // Always compress full frame
            self.compress_frame_full(&image).await?
        };
        
        // Update last frame for delta compression
        if should_use_delta {
            let mut last_frame = self.last_frame.write().await;
            *last_frame = Some(image);
        }
        
        // Update frame counter
        {
            let mut counter = self.frame_counter.write().await;
            *counter += 1;
        }
        
        let total_time = start_time.elapsed();
        debug!(
            "Frame processed: capture={}ms, convert={}ms, compress={}ms, total={}ms, size={}KB",
            capture_time.as_millis(),
            conversion_time.as_millis(),
            (total_time - conversion_time - capture_time).as_millis(),
            total_time.as_millis(),
            compressed_data.len() / 1024
        );
        
        Ok(compressed_data)
    }
    
    async fn compress_frame_full(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        let compressor = self.compressor.read().await;
        compressor.compress_frame(image).await
    }
    
    async fn compress_frame_delta(&self, current: &RgbaImage, previous: &RgbaImage) -> Result<Vec<u8>> {
        let compressor = self.compressor.read().await;
        compressor.compress_frame_delta(current, previous).await
    }
    
    fn bytes_to_image(&self, bytes: Vec<u8>) -> Result<RgbaImage> {
        // Get primary screen dimensions
        let screens = Screen::all()?;
        let primary_screen = screens.first()
            .ok_or_else(|| anyhow::anyhow!("No screens available"))?;
        
        let width = primary_screen.display_info.width as u32;
        let height = primary_screen.display_info.height as u32;
        
        // Convert RGBA bytes to image
        ImageBuffer::from_raw(width, height, bytes)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from bytes"))
    }
    
    pub async fn update_config(&mut self, new_config: StreamingConfig) -> Result<()> {
        info!("Updating screen streamer config");
        
        self.config = new_config.clone();
        
        // Update compressor
        let mut compressor = self.compressor.write().await;
        *compressor = Compressor::new(new_config)?;
        
        Ok(())
    }
    
    pub async fn set_quality(&mut self, quality: u8) -> Result<()> {
        self.config.quality = quality;
        
        let mut compressor = self.compressor.write().await;
        compressor.set_quality(quality).await?;
        
        Ok(())
    }
    
    pub async fn get_frame_count(&self) -> u64 {
        *self.frame_counter.read().await
    }
    
    pub async fn reset_frame_count(&self) {
        let mut counter = self.frame_counter.write().await;
        *counter = 0;
    }
    
    pub async fn capture_region(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Vec<u8>> {
        // Capture full screen first
        let full_screen = self.capture_manager.capture_primary_screen().await?;
        let full_image = self.bytes_to_image(full_screen)?;
        
        // Get screen dimensions
        let screen_width = full_image.width();
        let screen_height = full_image.height();
        
        // Validate region bounds
        if x >= screen_width || y >= screen_height {
            return Err(anyhow::anyhow!("Region coordinates out of bounds"));
        }
        
        let actual_width = width.min(screen_width - x);
        let actual_height = height.min(screen_height - y);
        
        // Extract region
        let mut region_image = ImageBuffer::new(actual_width, actual_height);
        
        for (region_x, region_y, pixel) in region_image.enumerate_pixels_mut() {
            let source_x = x + region_x;
            let source_y = y + region_y;
            
            if source_x < screen_width && source_y < screen_height {
                *pixel = *full_image.get_pixel(source_x, source_y);
            }
        }
        
        // Compress region
        let compressor = self.compressor.read().await;
        compressor.compress_frame(&region_image).await
    }
    
    pub fn get_config(&self) -> &StreamingConfig {
        &self.config
    }
}