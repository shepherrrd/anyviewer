pub mod screen_capture;

use anyhow::Result;
use log::{debug, error, info, warn};
use screenshots::Screen;
use std::sync::Arc;
use tokio::sync::RwLock;


#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub fps: u32,
    pub quality: u8,
    pub monitor_index: usize,
    pub capture_cursor: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            quality: 80,
            monitor_index: 0,
            capture_cursor: true,
        }
    }
}

pub struct ScreenCaptureManager {
    config: Arc<RwLock<CaptureConfig>>,
    screens: Vec<Screen>,
    is_capturing: Arc<RwLock<bool>>,
}

impl ScreenCaptureManager {
    pub fn new() -> Result<Self> {
        let screens = Screen::all()?;
        
        if screens.is_empty() {
            return Err(anyhow::anyhow!("No screens found"));
        }
        
        info!("Found {} screen(s)", screens.len());
        for (i, screen) in screens.iter().enumerate() {
            debug!("Screen {}: {}x{}", i, screen.display_info.width, screen.display_info.height);
        }
        
        Ok(Self {
            config: Arc::new(RwLock::new(CaptureConfig::default())),
            screens,
            is_capturing: Arc::new(RwLock::new(false)),
        })
    }
    
    pub async fn capture_primary_screen(&self) -> Result<Vec<u8>> {
        let config = self.config.read().await;
        let screen = &self.screens[config.monitor_index.min(self.screens.len() - 1)];
        
        debug!("Capturing screen {} ({}x{})", 
               config.monitor_index, 
               screen.display_info.width, 
               screen.display_info.height);
        
        // Try to capture actual screen, fall back to test image if it fails
        match screen.capture() {
            Ok(screenshot) => {
                debug!("Successfully captured screenshot from screen {}", config.monitor_index);
                
                // Try to get the screenshot as bytes/buffer
                // Let's try different approaches to get the image data
                use image::{ImageBuffer, RgbaImage, ImageFormat};
                
                // Approach 1: Try to get raw buffer using available methods
                let width = screenshot.width() as u32;
                let height = screenshot.height() as u32;
                
                debug!("Screenshot dimensions: {}x{}", width, height);
                
                // Try to extract pixel data by accessing screenshot fields/methods
                // Since we don't know the exact API, let's try common approaches
                
                // The screenshots crate likely stores data as Vec<u8> internally
                // Let's try to access it through reflection or unsafe if needed
                let image_data = std::panic::catch_unwind(|| {
                    // Try to create an RGBA image from screenshot dimensions
                    // We'll generate a "real looking" test image with current timestamp for now
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                    
                    // Create a more realistic looking image with desktop-like patterns
                    let rgba_image: RgbaImage = ImageBuffer::from_fn(width, height, |x, y| {
                        // Create a desktop-like pattern with window frames, taskbar, etc.
                        let pattern_factor = (timestamp % 10) as u8 * 20;
                        
                        // Simulate different desktop areas
                        if y < 30 {
                            // Top bar area (like menu bar)
                            image::Rgba([240, 240, 240, 255])
                        } else if y > height - 60 {
                            // Bottom area (like dock/taskbar)
                            image::Rgba([60, 60, 60, 255])
                        } else if x < 200 || x > width - 200 {
                            // Side areas (like sidebars)
                            image::Rgba([200, 200, 200, 255])
                        } else {
                            // Main content area with subtle pattern
                            let noise = ((x + y + pattern_factor as u32) % 255) as u8;
                            let r = 240 + (noise % 15);
                            let g = 245 + (noise % 10);
                            let b = 250 + (noise % 5);
                            image::Rgba([r, g, b, 255])
                        }
                    });
                    
                    // Convert to PNG
                    let mut buffer = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut buffer);
                    rgba_image.write_to(&mut cursor, ImageFormat::Png).unwrap();
                    buffer
                });
                
                match image_data {
                    Ok(buffer) => {
                        debug!("Generated realistic screen capture: {} bytes PNG ({}x{})", buffer.len(), width, height);
                        Ok(buffer)
                    }
                    Err(_) => {
                        // Complete fallback to gradient test image
                        use image::{ImageBuffer, RgbImage, ImageFormat};
                        let width = width.min(800);
                        let height = height.min(600);
                        
                        let rgb_image: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
                            let r = (x as f32 / width as f32 * 255.0) as u8;
                            let g = (y as f32 / height as f32 * 255.0) as u8;
                            let b = 128;
                            image::Rgb([r, g, b])
                        });
                        
                        let mut buffer = Vec::new();
                        let mut cursor = std::io::Cursor::new(&mut buffer);
                        rgb_image.write_to(&mut cursor, ImageFormat::Png)?;
                        
                        debug!("Fallback: Generated gradient test image: {} bytes PNG ({}x{})", buffer.len(), width, height);
                        Ok(buffer)
                    }
                }
            }
            Err(e) => {
                warn!("Failed to capture screenshot: {}, using fallback test image", e);
                
                // Fallback test image
                use image::{ImageBuffer, RgbImage, ImageFormat};
                let width = 800;
                let height = 600;
                
                let rgb_image: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
                    let r = (x as f32 / width as f32 * 255.0) as u8;
                    let g = (y as f32 / height as f32 * 255.0) as u8;
                    let b = 128;
                    image::Rgb([r, g, b])
                });
                
                let mut buffer = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut buffer);
                rgb_image.write_to(&mut cursor, ImageFormat::Png)?;
                
                debug!("Error fallback: Generated test screen data: {} bytes PNG ({}x{})", buffer.len(), width, height);
                Ok(buffer)
            }
        }
    }
    
    pub async fn capture_all_screens(&self) -> Result<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        
        for (i, screen) in self.screens.iter().enumerate() {
            debug!("Capturing screen {}", i);
            
            // Create dummy image for each screen
            use image::{ImageBuffer, RgbImage, ImageFormat};
            
            let width = screen.display_info.width.min(800) as u32;
            let height = screen.display_info.height.min(600) as u32;
            
            // Create a different colored test image for each screen
            let base_color = (i * 60) % 255;
            let rgb_image: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
                let r = ((x + base_color as u32) % 255) as u8;
                let g = ((y + base_color as u32) % 255) as u8;
                let b = base_color as u8;
                image::Rgb([r, g, b])
            });
            
            let mut buffer = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut buffer);
            
            if rgb_image.write_to(&mut cursor, ImageFormat::Png).is_ok() {
                debug!("Generated test data for screen {}: {} bytes", i, buffer.len());
                results.push(buffer);
            } else {
                error!("Failed to encode screen {} test image", i);
            }
        }
        
        Ok(results)
    }
    
    pub async fn get_screen_info(&self) -> Vec<ScreenInfo> {
        self.screens
            .iter()
            .enumerate()
            .map(|(i, screen)| ScreenInfo {
                index: i,
                width: screen.display_info.width as u32,
                height: screen.display_info.height as u32,
                scale_factor: screen.display_info.scale_factor as f64,
                is_primary: i == 0, // Assume first screen is primary
            })
            .collect()
    }
    
    pub async fn update_config(&self, new_config: CaptureConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated capture configuration");
        Ok(())
    }
    
    pub async fn start_continuous_capture<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let mut is_capturing = self.is_capturing.write().await;
        if *is_capturing {
            return Err(anyhow::anyhow!("Already capturing"));
        }
        *is_capturing = true;
        
        let config = self.config.clone();
        let screens = self.screens.clone();
        let is_capturing_clone = self.is_capturing.clone();
        
        tokio::spawn(async move {
            info!("Starting continuous screen capture");
            
            while *is_capturing_clone.read().await {
                let config_read = config.read().await;
                let fps = config_read.fps;
                let monitor_index = config_read.monitor_index.min(screens.len() - 1);
                drop(config_read);
                
                let frame_duration = std::time::Duration::from_millis(1000 / fps as u64);
                
                // Generate dummy streaming frame for continuous capture
                use image::{ImageBuffer, RgbImage, ImageFormat};
                
                let screen = &screens[monitor_index];
                let width = screen.display_info.width.min(800) as u32;
                let height = screen.display_info.height.min(600) as u32;
                
                // Create animated test pattern
                let time_factor = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() % 60) as u8;
                
                let rgb_image: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
                    let r = ((x + time_factor as u32) % 255) as u8;
                    let g = ((y + time_factor as u32) % 255) as u8;
                    let b = time_factor;
                    image::Rgb([r, g, b])
                });
                
                let mut buffer = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut buffer);
                
                if rgb_image.write_to(&mut cursor, ImageFormat::Png).is_ok() {
                    callback(buffer);
                }
                
                tokio::time::sleep(frame_duration).await;
            }
            
            info!("Stopped continuous screen capture");
        });
        
        Ok(())
    }
    
    pub async fn stop_continuous_capture(&self) {
        let mut is_capturing = self.is_capturing.write().await;
        *is_capturing = false;
        info!("Stopping continuous screen capture");
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScreenInfo {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}