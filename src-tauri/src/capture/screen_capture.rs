use anyhow::Result;
use image::RgbaImage;
use log::{debug, warn};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct CaptureStats {
    pub frames_captured: u64,
    pub bytes_captured: u64,
    pub average_fps: f64,
    pub last_capture_duration: Duration,
}

pub struct AdvancedScreenCapture {
    last_frame: Option<RgbaImage>,
    capture_stats: CaptureStats,
    last_capture_time: Instant,
}

impl AdvancedScreenCapture {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            capture_stats: CaptureStats {
                frames_captured: 0,
                bytes_captured: 0,
                average_fps: 0.0,
                last_capture_duration: Duration::ZERO,
            },
            last_capture_time: Instant::now(),
        }
    }
    
    /// Capture a specific region of the screen
    pub fn capture_region(&mut self, region: CaptureRegion) -> Result<Vec<u8>> {
        let start_time = Instant::now();
        
        // Get all screens and find the one containing the region
        let screens = screenshots::Screen::all()?;
        
        for screen in &screens {
            let display = &screen.display_info;
            
            // Check if region intersects with this screen
            if region.x < display.x + display.width as i32 &&
               region.x + region.width as i32 > display.x &&
               region.y < display.y + display.height as i32 &&
               region.y + region.height as i32 > display.y {
                
                debug!("Capturing region {:?} from screen at ({}, {})", region, display.x, display.y);
                
                let image = screen.capture()?;
                
                // Calculate the intersection and crop
                let crop_x = (region.x - display.x).max(0) as u32;
                let crop_y = (region.y - display.y).max(0) as u32;
                let crop_width = region.width.min(display.width - crop_x);
                let crop_height = region.height.min(display.height - crop_y);
                
                // For now, use the full image (cropping would need additional implementation)
                let cropped_image = image;
                
                // Convert to JPEG
                let mut buffer = Vec::new();
                // For now, just return empty buffer (would need proper Image to buffer conversion)
                // cropped_image.save_to(&mut cursor, image::ImageFormat::Jpeg)?;
                
                // Update stats
                self.update_stats(buffer.len(), start_time);
                
                return Ok(buffer);
            }
        }
        
        Err(anyhow::anyhow!("No screen found containing region {:?}", region))
    }
    
    /// Detect changes between current and previous frame
    pub fn detect_changes(&mut self, current_frame: &RgbaImage, threshold: u8) -> Vec<CaptureRegion> {
        let mut changed_regions = Vec::new();
        
        if let Some(ref last_frame) = self.last_frame {
            if last_frame.dimensions() != current_frame.dimensions() {
                warn!("Frame dimensions changed, capturing full screen");
                let (width, height) = current_frame.dimensions();
                changed_regions.push(CaptureRegion {
                    x: 0,
                    y: 0,
                    width,
                    height,
                });
            } else {
                // Simple change detection by comparing pixels
                // In a real implementation, you'd use more sophisticated algorithms
                let (width, height) = current_frame.dimensions();
                let block_size = 64u32; // 64x64 pixel blocks
                
                for y in (0..height).step_by(block_size as usize) {
                    for x in (0..width).step_by(block_size as usize) {
                        let block_width = block_size.min(width - x);
                        let block_height = block_size.min(height - y);
                        
                        if self.block_changed(last_frame, current_frame, x, y, block_width, block_height, threshold) {
                            changed_regions.push(CaptureRegion {
                                x: x as i32,
                                y: y as i32,
                                width: block_width,
                                height: block_height,
                            });
                        }
                    }
                }
            }
        }
        
        // Store current frame for next comparison
        self.last_frame = Some(current_frame.clone());
        
        changed_regions
    }
    
    fn block_changed(&self, old_frame: &RgbaImage, new_frame: &RgbaImage, 
                    x: u32, y: u32, width: u32, height: u32, threshold: u8) -> bool {
        let mut diff_pixels = 0;
        let total_pixels = width * height;
        
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;
                
                if px < old_frame.width() && py < old_frame.height() {
                    let old_pixel = old_frame.get_pixel(px, py);
                    let new_pixel = new_frame.get_pixel(px, py);
                    
                    // Simple RGB difference
                    let diff = ((old_pixel[0] as i16 - new_pixel[0] as i16).abs() +
                               (old_pixel[1] as i16 - new_pixel[1] as i16).abs() +
                               (old_pixel[2] as i16 - new_pixel[2] as i16).abs()) as u8;
                    
                    if diff > threshold {
                        diff_pixels += 1;
                    }
                }
            }
        }
        
        // Consider block changed if more than 5% of pixels changed
        (diff_pixels * 100 / total_pixels) > 5
    }
    
    fn update_stats(&mut self, bytes_captured: usize, start_time: Instant) {
        let capture_duration = start_time.elapsed();
        
        self.capture_stats.frames_captured += 1;
        self.capture_stats.bytes_captured += bytes_captured as u64;
        self.capture_stats.last_capture_duration = capture_duration;
        
        // Calculate FPS
        let time_since_last = self.last_capture_time.elapsed();
        if time_since_last.as_secs_f64() > 0.0 {
            self.capture_stats.average_fps = 1.0 / time_since_last.as_secs_f64();
        }
        
        self.last_capture_time = Instant::now();
        
        debug!("Capture stats: {} frames, {} bytes, {:.2} fps, {:.2}ms duration",
               self.capture_stats.frames_captured,
               self.capture_stats.bytes_captured,
               self.capture_stats.average_fps,
               capture_duration.as_millis());
    }
    
    pub fn get_stats(&self) -> &CaptureStats {
        &self.capture_stats
    }
    
    pub fn reset_stats(&mut self) {
        self.capture_stats = CaptureStats {
            frames_captured: 0,
            bytes_captured: 0,
            average_fps: 0.0,
            last_capture_duration: Duration::ZERO,
        };
    }
}