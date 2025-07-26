pub mod enhanced_input;

use anyhow::Result;
use enigo::{Enigo, Mouse, Keyboard, Settings, Direction, Button, Key, Coordinate};
use log::{debug, error, warn};
use std::sync::Arc;
use tokio::sync::RwLock;


#[derive(Debug, Clone)]
pub struct InputConfig {
    pub enable_mouse: bool,
    pub enable_keyboard: bool,
    pub mouse_acceleration: f64,
    pub keyboard_repeat_delay: u64,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            enable_mouse: true,
            enable_keyboard: true,
            mouse_acceleration: 1.0,
            keyboard_repeat_delay: 100,
        }
    }
}

pub struct InputManager {
    enigo: Enigo,
    config: Arc<RwLock<InputConfig>>,
}

impl InputManager {
    pub fn new() -> Self {
        let enigo = Enigo::new(&Settings::default()).unwrap_or_else(|e| {
            error!("Failed to initialize input manager: {}", e);
            // Create a dummy enigo instance - in a real app you'd handle this better
            Enigo::new(&Settings::default()).unwrap()
        });
        
        Self {
            enigo,
            config: Arc::new(RwLock::new(InputConfig::default())),
        }
    }
    
    pub async fn send_input(&mut self, x: i32, y: i32, event_type: String, data: String) -> Result<()> {
        let (enable_mouse, enable_keyboard) = {
            let config = self.config.read().await;
            (config.enable_mouse, config.enable_keyboard)
        };
        
        match event_type.as_str() {
            "mouse_move" => {
                if enable_mouse {
                    self.send_mouse_move(x, y).await?;
                }
            }
            "mouse_click" => {
                if enable_mouse {
                    self.send_mouse_click(x, y, &data).await?;
                }
            }
            "mouse_scroll" => {
                if enable_mouse {
                    self.send_mouse_scroll(x, y, &data).await?;
                }
            }
            "key_press" => {
                if enable_keyboard {
                    self.send_key_press(&data).await?;
                }
            }
            "key_type" => {
                if enable_keyboard {
                    self.send_key_type(&data).await?;
                }
            }
            _ => {
                warn!("Unknown input event type: {}", event_type);
            }
        }
        
        Ok(())
    }
    
    async fn send_mouse_move(&mut self, x: i32, y: i32) -> Result<()> {
        debug!("Moving mouse to ({}, {})", x, y);
        
        let config = self.config.read().await;
        let adjusted_x = (x as f64 * config.mouse_acceleration) as i32;
        let adjusted_y = (y as f64 * config.mouse_acceleration) as i32;
        drop(config);
        
        self.enigo.move_mouse(adjusted_x, adjusted_y, Coordinate::Abs)
            .map_err(|e| anyhow::anyhow!("Failed to move mouse: {}", e))?;
        
        Ok(())
    }
    
    async fn send_mouse_click(&mut self, x: i32, y: i32, button_data: &str) -> Result<()> {
        debug!("Mouse click at ({}, {}) with button: {}", x, y, button_data);
        
        // Move to position first
        self.send_mouse_move(x, y).await?;
        
        // Parse button
        let button = match button_data {
            "left" => Button::Left,
            "right" => Button::Right,
            "middle" => Button::Middle,
            _ => {
                warn!("Unknown mouse button: {}", button_data);
                Button::Left
            }
        };
        
        // Click
        self.enigo.button(button, Direction::Press)
            .map_err(|e| anyhow::anyhow!("Failed to press mouse button: {}", e))?;
        
        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        self.enigo.button(button, Direction::Release)
            .map_err(|e| anyhow::anyhow!("Failed to release mouse button: {}", e))?;
        
        Ok(())
    }
    
    async fn send_mouse_scroll(&mut self, _x: i32, _y: i32, scroll_data: &str) -> Result<()> {
        debug!("Mouse scroll: {}", scroll_data);
        
        // Parse scroll direction and amount
        let (direction, amount) = if scroll_data.starts_with("up") {
            (1, scroll_data.strip_prefix("up").unwrap_or("1").parse::<i32>().unwrap_or(1))
        } else if scroll_data.starts_with("down") {
            (-1, scroll_data.strip_prefix("down").unwrap_or("1").parse::<i32>().unwrap_or(1))
        } else {
            (0, 0)
        };
        
        if direction != 0 {
            self.enigo.scroll(direction * amount, enigo::Axis::Vertical)
                .map_err(|e| anyhow::anyhow!("Failed to scroll: {}", e))?;
        }
        
        Ok(())
    }
    
    async fn send_key_press(&mut self, key_data: &str) -> Result<()> {
        debug!("Key press: {}", key_data);
        
        let key = self.parse_key(key_data)?;
        
        self.enigo.key(key, Direction::Press)
            .map_err(|e| anyhow::anyhow!("Failed to press key: {}", e))?;
        
        // Add delay for key repeat
        let config = self.config.read().await;
        let delay = config.keyboard_repeat_delay;
        drop(config);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        
        self.enigo.key(key, Direction::Release)
            .map_err(|e| anyhow::anyhow!("Failed to release key: {}", e))?;
        
        Ok(())
    }
    
    async fn send_key_type(&mut self, text: &str) -> Result<()> {
        debug!("Typing text: {}", text);
        
        self.enigo.text(text)
            .map_err(|e| anyhow::anyhow!("Failed to type text: {}", e))?;
        
        Ok(())
    }
    
    fn parse_key(&self, key_str: &str) -> Result<Key> {
        let key = match key_str.to_lowercase().as_str() {
            // Letter keys
            "a" => Key::Unicode('a'),
            "b" => Key::Unicode('b'),
            "c" => Key::Unicode('c'),
            "d" => Key::Unicode('d'),
            "e" => Key::Unicode('e'),
            "f" => Key::Unicode('f'),
            "g" => Key::Unicode('g'),
            "h" => Key::Unicode('h'),
            "i" => Key::Unicode('i'),
            "j" => Key::Unicode('j'),
            "k" => Key::Unicode('k'),
            "l" => Key::Unicode('l'),
            "m" => Key::Unicode('m'),
            "n" => Key::Unicode('n'),
            "o" => Key::Unicode('o'),
            "p" => Key::Unicode('p'),
            "q" => Key::Unicode('q'),
            "r" => Key::Unicode('r'),
            "s" => Key::Unicode('s'),
            "t" => Key::Unicode('t'),
            "u" => Key::Unicode('u'),
            "v" => Key::Unicode('v'),
            "w" => Key::Unicode('w'),
            "x" => Key::Unicode('x'),
            "y" => Key::Unicode('y'),
            "z" => Key::Unicode('z'),
            
            // Number keys
            "0" => Key::Unicode('0'),
            "1" => Key::Unicode('1'),
            "2" => Key::Unicode('2'),
            "3" => Key::Unicode('3'),
            "4" => Key::Unicode('4'),
            "5" => Key::Unicode('5'),
            "6" => Key::Unicode('6'),
            "7" => Key::Unicode('7'),
            "8" => Key::Unicode('8'),
            "9" => Key::Unicode('9'),
            
            // Special keys
            "space" => Key::Unicode(' '),
            "enter" => Key::Return,
            "return" => Key::Return,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "escape" => Key::Escape,
            "esc" => Key::Escape,
            
            // Arrow keys
            "up" => Key::UpArrow,
            "down" => Key::DownArrow,
            "left" => Key::LeftArrow,
            "right" => Key::RightArrow,
            
            // Modifier keys
            "shift" => Key::Shift,
            "ctrl" => Key::Control,
            "control" => Key::Control,
            "alt" => Key::Alt,
            "meta" => Key::Meta,
            "cmd" => Key::Meta,
            "super" => Key::Meta,
            
            // Function keys
            "f1" => Key::F1,
            "f2" => Key::F2,
            "f3" => Key::F3,
            "f4" => Key::F4,
            "f5" => Key::F5,
            "f6" => Key::F6,
            "f7" => Key::F7,
            "f8" => Key::F8,
            "f9" => Key::F9,
            "f10" => Key::F10,
            "f11" => Key::F11,
            "f12" => Key::F12,
            
            // Other common keys
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            // "insert" => Key::Insert, // Not available in this version of enigo
            
            _ => {
                // Try to parse as single character
                if key_str.len() == 1 {
                    Key::Unicode(key_str.chars().next().unwrap())
                } else {
                    return Err(anyhow::anyhow!("Unknown key: {}", key_str));
                }
            }
        };
        
        Ok(key)
    }
    
    pub async fn update_config(&self, new_config: InputConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        debug!("Updated input configuration");
        Ok(())
    }
    
    pub async fn get_config(&self) -> InputConfig {
        self.config.read().await.clone()
    }
    
    pub async fn is_input_available(&self) -> bool {
        // Check if input subsystem is available
        // This is a simple check - in a real implementation you might want to
        // check for specific permissions or capabilities
        true
    }
}