use anyhow::Result;
use image::{ImageBuffer, RgbaImage, ImageFormat};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecConfig {
    pub format: CompressionFormat,
    pub quality: u8, // 1-100
    pub enable_delta_compression: bool,
    pub max_frame_size: usize,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            format: CompressionFormat::Jpeg,
            quality: 80,
            enable_delta_compression: true,
            max_frame_size: 5 * 1024 * 1024, // 5MB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionFormat {
    Jpeg,
    Png,
    WebP,
    Raw,
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub format: CompressionFormat,
    pub quality: u8,
    pub size: usize,
    pub compression_ratio: f64,
}

pub struct VideoCodec {
    config: CodecConfig,
    last_frame: Option<RgbaImage>,
    frame_counter: u64,
}

impl VideoCodec {
    pub fn new(config: CodecConfig) -> Self {
        debug!("Initializing video codec with config: {:?}", config);
        
        Self {
            config,
            last_frame: None,
            frame_counter: 0,
        }
    }
    
    pub fn encode_frame(&mut self, image: &RgbaImage) -> Result<(Vec<u8>, FrameInfo)> {
        self.frame_counter += 1;
        
        let original_size = (image.width() * image.height() * 4) as usize;
        debug!("Encoding frame {} ({}x{}, {} bytes)", 
               self.frame_counter, image.width(), image.height(), original_size);
        
        let encoded_data = match self.config.format {
            CompressionFormat::Jpeg => self.encode_jpeg(image)?,
            CompressionFormat::Png => self.encode_png(image)?,
            CompressionFormat::WebP => self.encode_webp(image)?,
            CompressionFormat::Raw => self.encode_raw(image)?,
        };
        
        let compression_ratio = original_size as f64 / encoded_data.len() as f64;
        
        let frame_info = FrameInfo {
            width: image.width(),
            height: image.height(),
            format: self.config.format.clone(),
            quality: self.config.quality,
            size: encoded_data.len(),
            compression_ratio,
        };
        
        debug!("Encoded frame: {} bytes, compression ratio: {:.2}x", 
               encoded_data.len(), compression_ratio);
        
        // Store frame for delta compression
        if self.config.enable_delta_compression {
            self.last_frame = Some(image.clone());
        }
        
        Ok((encoded_data, frame_info))
    }
    
    fn encode_jpeg(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // Convert RGBA to RGB for JPEG
        let rgb_image = self.rgba_to_rgb(image);
        
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        
        // Use image crate's JPEG encoder with quality setting
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, self.config.quality);
        encoder.encode(
            rgb_image.as_raw(),
            rgb_image.width(),
            rgb_image.height(),
            image::ColorType::Rgb8,
        )?;
        
        Ok(buffer)
    }
    
    fn encode_png(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        
        image.write_to(&mut cursor, ImageFormat::Png)?;
        
        Ok(buffer)
    }
    
    fn encode_webp(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        // WebP encoding would require additional dependencies like libwebp-sys
        // For now, fall back to JPEG
        warn!("WebP encoding not implemented, falling back to JPEG");
        self.encode_jpeg(image)
    }
    
    fn encode_raw(&self, image: &RgbaImage) -> Result<Vec<u8>> {
        Ok(image.as_raw().clone())
    }
    
    pub fn decode_frame(&self, data: &[u8], format: CompressionFormat) -> Result<RgbaImage> {
        debug!("Decoding frame ({} bytes, format: {:?})", data.len(), format);
        
        match format {
            CompressionFormat::Jpeg => self.decode_jpeg(data),
            CompressionFormat::Png => self.decode_png(data),
            CompressionFormat::WebP => self.decode_webp(data),
            CompressionFormat::Raw => self.decode_raw(data),
        }
    }
    
    fn decode_jpeg(&self, data: &[u8]) -> Result<RgbaImage> {
        let cursor = Cursor::new(data);
        let dynamic_image = image::load(cursor, ImageFormat::Jpeg)?;
        Ok(dynamic_image.to_rgba8())
    }
    
    fn decode_png(&self, data: &[u8]) -> Result<RgbaImage> {
        let cursor = Cursor::new(data);
        let dynamic_image = image::load(cursor, ImageFormat::Png)?;
        Ok(dynamic_image.to_rgba8())
    }
    
    fn decode_webp(&self, data: &[u8]) -> Result<RgbaImage> {
        // WebP decoding would require additional dependencies
        // For now, try to decode as JPEG
        warn!("WebP decoding not implemented, attempting JPEG decode");
        self.decode_jpeg(data)
    }
    
    fn decode_raw(&self, data: &[u8]) -> Result<RgbaImage> {
        // For raw data, we need to know the dimensions
        // This is a simplified implementation
        let pixel_count = data.len() / 4;
        let width = (pixel_count as f64).sqrt() as u32;
        let height = if width * width * 4 == data.len() as u32 {
            width
        } else {
            return Err(anyhow::anyhow!("Cannot determine image dimensions from raw data"));
        };
        
        ImageBuffer::from_raw(width, height, data.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw data"))
    }
    
    fn rgba_to_rgb(&self, rgba_image: &RgbaImage) -> ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        let (width, height) = rgba_image.dimensions();
        let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
        
        for pixel in rgba_image.pixels() {
            rgb_data.push(pixel[0]); // R
            rgb_data.push(pixel[1]); // G
            rgb_data.push(pixel[2]); // B
            // Skip alpha channel
        }
        
        ImageBuffer::from_raw(width, height, rgb_data)
            .expect("Failed to create RGB image")
    }
    
    pub fn encode_delta_frame(&mut self, current_frame: &RgbaImage) -> Result<Option<(Vec<u8>, FrameInfo)>> {
        if !self.config.enable_delta_compression {
            return Ok(None);
        }
        
        let Some(ref last_frame) = self.last_frame else {
            // No previous frame, encode full frame
            return Ok(Some(self.encode_frame(current_frame)?));
        };
        
        if last_frame.dimensions() != current_frame.dimensions() {
            // Dimensions changed, encode full frame
            return Ok(Some(self.encode_frame(current_frame)?));
        }
        
        // Calculate differences
        let diff_regions = self.calculate_differences(last_frame, current_frame)?;
        
        if diff_regions.is_empty() {
            // No changes, return None
            debug!("No changes detected, skipping frame");
            return Ok(None);
        }
        
        // For simplicity, if there are many changes, encode full frame
        let change_ratio = diff_regions.len() as f64 / (current_frame.width() * current_frame.height()) as f64;
        if change_ratio > 0.3 {
            debug!("High change ratio ({:.2}), encoding full frame", change_ratio);
            return Ok(Some(self.encode_frame(current_frame)?));
        }
        
        // Encode only changed regions (simplified implementation)
        debug!("Encoding delta frame with {} changed regions", diff_regions.len());
        Ok(Some(self.encode_frame(current_frame)?))
    }
    
    fn calculate_differences(&self, old_frame: &RgbaImage, new_frame: &RgbaImage) -> Result<Vec<DiffRegion>> {
        let mut diff_regions = Vec::new();
        let (width, height) = old_frame.dimensions();
        let block_size = 16u32; // 16x16 pixel blocks
        
        for y in (0..height).step_by(block_size as usize) {
            for x in (0..width).step_by(block_size as usize) {
                let block_width = block_size.min(width - x);
                let block_height = block_size.min(height - y);
                
                if self.block_differs(old_frame, new_frame, x, y, block_width, block_height) {
                    diff_regions.push(DiffRegion {
                        x,
                        y,
                        width: block_width,
                        height: block_height,
                    });
                }
            }
        }
        
        Ok(diff_regions)
    }
    
    fn block_differs(&self, old_frame: &RgbaImage, new_frame: &RgbaImage, 
                    x: u32, y: u32, width: u32, height: u32) -> bool {
        let threshold = 30; // Pixel difference threshold
        let mut diff_pixels = 0;
        let total_pixels = width * height;
        
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;
                
                if px < old_frame.width() && py < old_frame.height() {
                    let old_pixel = old_frame.get_pixel(px, py);
                    let new_pixel = new_frame.get_pixel(px, py);
                    
                    let diff = ((old_pixel[0] as i16 - new_pixel[0] as i16).abs() +
                               (old_pixel[1] as i16 - new_pixel[1] as i16).abs() +
                               (old_pixel[2] as i16 - new_pixel[2] as i16).abs()) as u8;
                    
                    if diff > threshold {
                        diff_pixels += 1;
                    }
                }
            }
        }
        
        // Consider block different if more than 10% of pixels changed
        (diff_pixels * 100 / total_pixels) > 10
    }
    
    pub fn update_config(&mut self, new_config: CodecConfig) -> Result<()> {
        debug!("Updating codec configuration: {:?}", new_config);
        self.config = new_config;
        Ok(())
    }
    
    pub fn get_stats(&self) -> CodecStats {
        CodecStats {
            frames_encoded: self.frame_counter,
            current_format: self.config.format.clone(),
            current_quality: self.config.quality,
            delta_compression_enabled: self.config.enable_delta_compression,
        }
    }
}

#[derive(Debug, Clone)]
struct DiffRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodecStats {
    pub frames_encoded: u64,
    pub current_format: CompressionFormat,
    pub current_quality: u8,
    pub delta_compression_enabled: bool,
}