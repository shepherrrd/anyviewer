use anyhow::Result;
use image::{RgbaImage, DynamicImage, ImageFormat};
use log::{debug, warn};
use std::io::Cursor;

use super::{StreamingConfig, CompressionType};

pub struct Compressor {
    config: StreamingConfig,
    jpeg_quality: u8,
    webp_quality: f32,
}

impl Compressor {
    pub fn new(config: StreamingConfig) -> Result<Self> {
        let jpeg_quality = config.quality;
        let webp_quality = config.quality as f32;
        
        Ok(Self {
            config,
            jpeg_quality,
            webp_quality,
        })
    }
    
    pub async fn compress_frame(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        match self.config.compression_type {
            CompressionType::JPEG => self.compress_jpeg(image).await,
            CompressionType::WebP => self.compress_webp(image).await,
            CompressionType::H264 => self.compress_h264(image).await,
            CompressionType::VP8 => self.compress_vp8(image).await,
            CompressionType::AV1 => self.compress_av1(image).await,
        }
    }
    
    pub async fn compress_frame_delta(&self, current: &RgbaImage, previous: &RgbaImage) -> Result<Vec<u8>> {
        // Create delta frame by comparing pixels
        let width = current.width();
        let height = current.height();
        
        if width != previous.width() || height != previous.height() {
            // Dimensions changed, compress full frame
            return self.compress_frame(current).await;
        }
        
        // Find changed regions using block-based approach
        let block_size = 16; // 16x16 pixel blocks
        let mut changed_blocks = Vec::new();
        
        for y in (0..height).step_by(block_size) {
            for x in (0..width).step_by(block_size) {
                let block_width = block_size.min((width - x) as usize) as u32;
                let block_height = block_size.min((height - y) as usize) as u32;
                
                if self.block_changed(current, previous, x, y, block_width, block_height) {
                    changed_blocks.push((x, y, block_width, block_height));
                }
            }
        }
        
        if changed_blocks.is_empty() {
            // No changes, return minimal frame
            return Ok(self.create_no_change_frame().await?);
        }
        
        // If too many blocks changed, compress full frame
        let total_blocks = ((width + block_size as u32 - 1) / block_size as u32) * 
                          ((height + block_size as u32 - 1) / block_size as u32);
        let changed_ratio = changed_blocks.len() as f32 / total_blocks as f32;
        
        if changed_ratio > 0.5 {
            debug!("Too many blocks changed ({}%), using full frame compression", (changed_ratio * 100.0) as u32);
            return self.compress_frame(current).await;
        }
        
        // Create delta frame with only changed blocks
        self.create_delta_frame(current, &changed_blocks).await
    }
    
    async fn compress_jpeg(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        let dynamic_image = DynamicImage::ImageRgba8(image.clone());
        let rgb_image = dynamic_image.to_rgb8();
        
        let mut cursor = Cursor::new(Vec::new());
        
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, self.jpeg_quality);
        encoder.encode(
            rgb_image.as_raw(),
            rgb_image.width(),
            rgb_image.height(),
            image::ColorType::Rgb8,
        )?;
        
        Ok(cursor.into_inner())
    }
    
    async fn compress_webp(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // WebP compression would require additional dependency
        // For now, fallback to JPEG
        warn!("WebP compression not implemented, falling back to JPEG");
        self.compress_jpeg(image).await
    }
    
    async fn compress_h264(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // H.264 compression would require ffmpeg integration
        // For now, fallback to JPEG
        warn!("H.264 compression not implemented, falling back to JPEG");
        self.compress_jpeg(image).await
    }
    
    async fn compress_vp8(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // VP8 compression would require additional dependency
        // For now, fallback to JPEG
        warn!("VP8 compression not implemented, falling back to JPEG");
        self.compress_jpeg(image).await
    }
    
    async fn compress_av1(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // AV1 compression would require additional dependency
        // For now, fallback to JPEG
        warn!("AV1 compression not implemented, falling back to JPEG");
        self.compress_jpeg(image).await
    }
    
    fn block_changed(&self, current: &RgbaImage, previous: &RgbaImage, x: u32, y: u32, width: u32, height: u32) -> bool {
        let threshold = 10; // Pixel difference threshold
        let mut different_pixels = 0;
        let total_pixels = width * height;
        
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;
                
                if px < current.width() && py < current.height() {
                    let current_pixel = current.get_pixel(px, py);
                    let previous_pixel = previous.get_pixel(px, py);
                    
                    // Calculate pixel difference (simple RGB distance)
                    let r_diff = (current_pixel[0] as i32 - previous_pixel[0] as i32).abs();
                    let g_diff = (current_pixel[1] as i32 - previous_pixel[1] as i32).abs();
                    let b_diff = (current_pixel[2] as i32 - previous_pixel[2] as i32).abs();
                    
                    if r_diff > threshold || g_diff > threshold || b_diff > threshold {
                        different_pixels += 1;
                    }
                }
            }
        }
        
        // Block is considered changed if more than 5% of pixels are different
        let change_ratio = different_pixels as f32 / total_pixels as f32;
        change_ratio > 0.05
    }
    
    async fn create_no_change_frame(&self) -> Result<Vec<u8>> {
        // Create a minimal frame indicating no changes
        let delta_header = DeltaFrameHeader {
            frame_type: DeltaFrameType::NoChange,
            block_count: 0,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        };
        
        Ok(bincode::serialize(&delta_header)?)
    }
    
    async fn create_delta_frame(&self, image: &RgbaImage, changed_blocks: &[(u32, u32, u32, u32)]) -> Result<Vec<u8>> {
        let mut delta_data = Vec::new();
        
        // Add delta frame header
        let delta_header = DeltaFrameHeader {
            frame_type: DeltaFrameType::Delta,
            block_count: changed_blocks.len() as u32,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        };
        
        let header_bytes = bincode::serialize(&delta_header)?;
        delta_data.extend_from_slice(&header_bytes);
        
        // Add each changed block
        for &(x, y, width, height) in changed_blocks {
            // Extract block from image
            let mut block_image = image::ImageBuffer::new(width, height);
            
            for (bx, by, pixel) in block_image.enumerate_pixels_mut() {
                let source_x = x + bx;
                let source_y = y + by;
                
                if source_x < image.width() && source_y < image.height() {
                    *pixel = *image.get_pixel(source_x, source_y);
                }
            }
            
            // Compress block
            let compressed_block = self.compress_jpeg(&block_image).await?;
            
            // Add block metadata
            let block_info = DeltaBlock {
                x,
                y,
                width,
                height,
                data_size: compressed_block.len() as u32,
            };
            
            let block_info_bytes = bincode::serialize(&block_info)?;
            delta_data.extend_from_slice(&block_info_bytes);
            delta_data.extend_from_slice(&compressed_block);
        }
        
        debug!("Created delta frame with {} blocks, total size: {}KB", 
               changed_blocks.len(), delta_data.len() / 1024);
        
        Ok(delta_data)
    }
    
    pub async fn set_quality(&mut self, quality: u8) -> Result<()> {
        if quality == 0 || quality > 100 {
            return Err(anyhow::anyhow!("Quality must be between 1 and 100"));
        }
        
        self.jpeg_quality = quality;
        self.webp_quality = quality as f32;
        self.config.quality = quality;
        
        debug!("Updated compression quality to {}", quality);
        Ok(())
    }
    
    pub fn get_compression_info(&self) -> CompressionInfo {
        CompressionInfo {
            compression_type: self.config.compression_type.clone(),
            quality: self.config.quality,
            supports_delta: true,
            supports_adaptive: self.config.adaptive_quality,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaFrameHeader {
    pub frame_type: DeltaFrameType,
    pub block_count: u32,
    pub timestamp: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DeltaFrameType {
    Full,
    Delta,
    NoChange,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaBlock {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub data_size: u32,
}

#[derive(Debug, Clone)]
pub struct CompressionInfo {
    pub compression_type: CompressionType,
    pub quality: u8,
    pub supports_delta: bool,
    pub supports_adaptive: bool,
}

// Decompression functions for the client side
pub async fn decompress_frame(data: &[u8], compression_type: CompressionType) -> Result<RgbaImage> {
    match compression_type {
        CompressionType::JPEG => decompress_jpeg(data).await,
        CompressionType::WebP => decompress_webp(data).await,
        _ => {
            warn!("Compression type not implemented for decompression, trying JPEG");
            decompress_jpeg(data).await
        }
    }
}

async fn decompress_jpeg(data: &[u8]) -> Result<RgbaImage> {
    let image = image::load_from_memory_with_format(data, ImageFormat::Jpeg)?;
    Ok(image.to_rgba8())
}

async fn decompress_webp(data: &[u8]) -> Result<RgbaImage> {
    // WebP decompression would require additional dependency
    warn!("WebP decompression not implemented, trying JPEG");
    decompress_jpeg(data).await
}

pub async fn apply_delta_frame(base_frame: &mut RgbaImage, delta_data: &[u8]) -> Result<()> {
    let mut cursor = std::io::Cursor::new(delta_data);
    
    // Read header
    let header: DeltaFrameHeader = bincode::deserialize_from(&mut cursor)?;
    
    match header.frame_type {
        DeltaFrameType::NoChange => {
            // No changes to apply
            return Ok(());
        }
        DeltaFrameType::Full => {
            // This should be handled as a full frame, not delta
            return Err(anyhow::anyhow!("Received full frame in delta context"));
        }
        DeltaFrameType::Delta => {
            // Apply delta blocks
            for _ in 0..header.block_count {
                let block_info: DeltaBlock = bincode::deserialize_from(&mut cursor)?;
                
                // Read block data
                let mut block_data = vec![0u8; block_info.data_size as usize];
                std::io::Read::read_exact(&mut cursor, &mut block_data)?;
                
                // Decompress block
                let block_image = decompress_jpeg(&block_data).await?;
                
                // Apply block to base frame
                for (bx, by, pixel) in block_image.enumerate_pixels() {
                    let target_x = block_info.x + bx;
                    let target_y = block_info.y + by;
                    
                    if target_x < base_frame.width() && target_y < base_frame.height() {
                        base_frame.put_pixel(target_x, target_y, *pixel);
                    }
                }
            }
        }
    }
    
    Ok(())
}