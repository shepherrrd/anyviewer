use anyhow::Result;
use enigo::{Enigo, Mouse, Keyboard, Settings, Direction, Button, Key, Coordinate};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub event_type: MouseEventType,
    pub button: Option<MouseButtonType>,
    pub delta: Option<i32>, // For scroll events
    pub modifiers: Vec<KeyModifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEventType {
    Move,
    Press,
    Release,
    Click,
    DoubleClick,
    Scroll,
    Drag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    pub event_type: KeyboardEventType,
    pub key: Option<String>,
    pub key_code: Option<u32>,
    pub text: Option<String>,
    pub modifiers: Vec<KeyModifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyboardEventType {
    KeyDown,
    KeyUp,
    KeyPress, // Combined down+up
    TextInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
    Super,
}

#[derive(Debug, Clone)]
pub struct InputConfig {
    pub enable_mouse: bool,
    pub enable_keyboard: bool,
    pub mouse_acceleration: f64,
    pub double_click_speed: u64, // milliseconds
    pub key_repeat_delay: u64,
    pub key_repeat_rate: u64,
    pub smooth_mouse_movement: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            enable_mouse: true,
            enable_keyboard: true,
            mouse_acceleration: 1.0,
            double_click_speed: 500,
            key_repeat_delay: 250,
            key_repeat_rate: 33,
            smooth_mouse_movement: true,
        }
    }
}

pub struct EnhancedInputManager {
    enigo: Enigo,
    config: Arc<RwLock<InputConfig>>,
    mouse_state: Arc<RwLock<MouseState>>,
    keyboard_state: Arc<RwLock<KeyboardState>>,
}

#[derive(Debug, Clone)]
struct MouseState {
    last_position: (i32, i32),
    last_click_time: Option<Instant>,
    last_click_position: Option<(i32, i32)>,
    pressed_buttons: HashMap<MouseButtonType, bool>,
    drag_state: Option<DragState>,
}

#[derive(Debug, Clone)]
struct DragState {
    start_position: (i32, i32),
    current_position: (i32, i32),
    button: MouseButtonType,
    started_at: Instant,
}

#[derive(Debug, Clone)]
struct KeyboardState {
    pressed_keys: HashMap<String, bool>,
    active_modifiers: Vec<KeyModifier>,
    last_key_time: Option<Instant>,
}

impl EnhancedInputManager {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())?;
        
        Ok(Self {
            enigo,
            config: Arc::new(RwLock::new(InputConfig::default())),
            mouse_state: Arc::new(RwLock::new(MouseState {
                last_position: (0, 0),
                last_click_time: None,
                last_click_position: None,
                pressed_buttons: HashMap::new(),
                drag_state: None,
            })),
            keyboard_state: Arc::new(RwLock::new(KeyboardState {
                pressed_keys: HashMap::new(),
                active_modifiers: Vec::new(),
                last_key_time: None,
            })),
        })
    }
    
    /// Handle mouse events with enhanced functionality
    pub async fn handle_mouse_event(&mut self, event: MouseEvent) -> Result<()> {
        let mouse_enabled = {
            let config = self.config.read().await;
            config.enable_mouse
        };
        
        if !mouse_enabled {
            return Ok(());
        }
        
        debug!("Processing mouse event: {:?}", event);
        
        match event.event_type {
            MouseEventType::Move => {
                let config = self.config.read().await.clone();
                self.handle_mouse_move(event.x, event.y, &config).await?;
            },
            MouseEventType::Press => {
                if let Some(button) = event.button {
                    self.handle_mouse_press(event.x, event.y, button).await?;
                }
            },
            MouseEventType::Release => {
                if let Some(button) = event.button {
                    self.handle_mouse_release(event.x, event.y, button).await?;
                }
            },
            MouseEventType::Click => {
                if let Some(button) = event.button {
                    let config = self.config.read().await.clone();
                    self.handle_mouse_click(event.x, event.y, button, &config).await?;
                }
            },
            MouseEventType::DoubleClick => {
                if let Some(button) = event.button {
                    self.handle_mouse_double_click(event.x, event.y, button).await?;
                }
            },
            MouseEventType::Scroll => {
                if let Some(delta) = event.delta {
                    self.handle_mouse_scroll(event.x, event.y, delta).await?;
                }
            },
            MouseEventType::Drag => {
                if let Some(button) = event.button {
                    self.handle_mouse_drag(event.x, event.y, button).await?;
                }
            },
        }
        
        Ok(())
    }
    
    async fn handle_mouse_move(&mut self, x: i32, y: i32, config: &InputConfig) -> Result<()> {
        let adjusted_x = (x as f64 * config.mouse_acceleration) as i32;
        let adjusted_y = (y as f64 * config.mouse_acceleration) as i32;
        
        if config.smooth_mouse_movement {
            // Implement smooth movement
            let mut mouse_state = self.mouse_state.write().await;
            let (last_x, last_y) = mouse_state.last_position;
            
            let steps = ((adjusted_x - last_x).abs().max((adjusted_y - last_y).abs()) / 10).max(1);
            
            for i in 1..=steps {
                let progress = i as f64 / steps as f64;
                let intermediate_x = last_x + ((adjusted_x - last_x) as f64 * progress) as i32;
                let intermediate_y = last_y + ((adjusted_y - last_y) as f64 * progress) as i32;
                
                self.enigo.move_mouse(intermediate_x, intermediate_y, Coordinate::Abs)?;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            
            mouse_state.last_position = (adjusted_x, adjusted_y);
        } else {
            self.enigo.move_mouse(adjusted_x, adjusted_y, Coordinate::Abs)?;
            self.mouse_state.write().await.last_position = (adjusted_x, adjusted_y);
        }
        
        debug!("Mouse moved to ({}, {})", adjusted_x, adjusted_y);
        Ok(())
    }
    
    async fn handle_mouse_press(&mut self, x: i32, y: i32, button: MouseButtonType) -> Result<()> {
        // Move to position first
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        
        let enigo_button = self.convert_mouse_button(button.clone());
        self.enigo.button(enigo_button, Direction::Press)?;
        
        // Update state
        let mut mouse_state = self.mouse_state.write().await;
        mouse_state.pressed_buttons.insert(button.clone(), true);
        mouse_state.last_position = (x, y);
        
        debug!("Mouse button {:?} pressed at ({}, {})", button, x, y);
        Ok(())
    }
    
    async fn handle_mouse_release(&mut self, x: i32, y: i32, button: MouseButtonType) -> Result<()> {
        let enigo_button = self.convert_mouse_button(button.clone());
        self.enigo.button(enigo_button, Direction::Release)?;
        
        // Update state
        let mut mouse_state = self.mouse_state.write().await;
        mouse_state.pressed_buttons.remove(&button);
        
        // Check if this was a drag operation
        if let Some(drag_state) = &mouse_state.drag_state {
            if drag_state.button == button {
                info!("Drag operation completed: {:?} to ({}, {})", 
                      drag_state.start_position, x, y);
                mouse_state.drag_state = None;
            }
        }
        
        debug!("Mouse button {:?} released at ({}, {})", button, x, y);
        Ok(())
    }
    
    async fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButtonType, config: &InputConfig) -> Result<()> {
        // Check for double-click
        let mut mouse_state = self.mouse_state.write().await;
        let now = Instant::now();
        
        let is_double_click = if let (Some(last_time), Some(last_pos)) = 
            (mouse_state.last_click_time, mouse_state.last_click_position) {
            
            let time_diff = now.duration_since(last_time).as_millis() as u64;
            let distance = ((x - last_pos.0).pow(2) + (y - last_pos.1).pow(2)) as f64;
            
            time_diff < config.double_click_speed && distance < 25.0 // 5 pixel radius
        } else {
            false
        };
        
        drop(mouse_state);
        
        if is_double_click {
            self.handle_mouse_double_click(x, y, button).await?;
            return Ok(());
        }
        
        // Regular click
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        
        let enigo_button = self.convert_mouse_button(button.clone());
        self.enigo.button(enigo_button, Direction::Press)?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.enigo.button(enigo_button, Direction::Release)?;
        
        // Update click state
        let mut mouse_state = self.mouse_state.write().await;
        mouse_state.last_click_time = Some(now);
        mouse_state.last_click_position = Some((x, y));
        mouse_state.last_position = (x, y);
        
        debug!("Mouse clicked at ({}, {}) with button {:?}", x, y, button);
        Ok(())
    }
    
    async fn handle_mouse_double_click(&mut self, x: i32, y: i32, button: MouseButtonType) -> Result<()> {
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        
        let enigo_button = self.convert_mouse_button(button.clone());
        
        // First click
        self.enigo.button(enigo_button, Direction::Press)?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.enigo.button(enigo_button, Direction::Release)?;
        
        // Small delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Second click
        self.enigo.button(enigo_button, Direction::Press)?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.enigo.button(enigo_button, Direction::Release)?;
        
        info!("Double-click performed at ({}, {}) with button {:?}", x, y, button);
        Ok(())
    }
    
    async fn handle_mouse_scroll(&mut self, _x: i32, _y: i32, delta: i32) -> Result<()> {
        // Normalize scroll delta
        let scroll_amount = if delta > 0 { 3 } else { -3 };
        
        self.enigo.scroll(scroll_amount, enigo::Axis::Vertical)?;
        
        debug!("Mouse scrolled: delta={}, amount={}", delta, scroll_amount);
        Ok(())
    }
    
    async fn handle_mouse_drag(&mut self, x: i32, y: i32, button: MouseButtonType) -> Result<()> {
        let mut mouse_state = self.mouse_state.write().await;
        
        if mouse_state.drag_state.is_none() {
            // Start drag
            mouse_state.drag_state = Some(DragState {
                start_position: mouse_state.last_position,
                current_position: (x, y),
                button: button.clone(),
                started_at: Instant::now(),
            });
            
            debug!("Started drag from {:?} with button {:?}", mouse_state.last_position, button);
        } else {
            // Continue drag
            if let Some(ref mut drag_state) = mouse_state.drag_state {
                drag_state.current_position = (x, y);
            }
        }
        
        mouse_state.last_position = (x, y);
        drop(mouse_state);
        
        // Move mouse to new position
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        
        Ok(())
    }
    
    /// Handle keyboard events with enhanced functionality
    pub async fn handle_keyboard_event(&mut self, event: KeyboardEvent) -> Result<()> {
        let keyboard_enabled = {
            let config = self.config.read().await;
            config.enable_keyboard
        };
        
        if !keyboard_enabled {
            return Ok(());
        }
        
        debug!("Processing keyboard event: {:?}", event);
        
        // Apply modifiers first
        self.apply_modifiers(&event.modifiers).await?;
        
        match event.event_type {
            KeyboardEventType::KeyDown => {
                if let Some(key) = &event.key {
                    self.handle_key_down(key).await?;
                }
            },
            KeyboardEventType::KeyUp => {
                if let Some(key) = &event.key {
                    self.handle_key_up(key).await?;
                }
            },
            KeyboardEventType::KeyPress => {
                if let Some(key) = &event.key {
                    let config = self.config.read().await.clone();
                    self.handle_key_press(key, &config).await?;
                }
            },
            KeyboardEventType::TextInput => {
                if let Some(text) = &event.text {
                    self.handle_text_input(text).await?;
                }
            },
        }
        
        Ok(())
    }
    
    async fn handle_key_down(&mut self, key: &str) -> Result<()> {
        let enigo_key = self.convert_key(key)?;
        self.enigo.key(enigo_key, Direction::Press)?;
        
        // Update state
        let mut keyboard_state = self.keyboard_state.write().await;
        keyboard_state.pressed_keys.insert(key.to_string(), true);
        keyboard_state.last_key_time = Some(Instant::now());
        
        debug!("Key '{}' pressed", key);
        Ok(())
    }
    
    async fn handle_key_up(&mut self, key: &str) -> Result<()> {
        let enigo_key = self.convert_key(key)?;
        self.enigo.key(enigo_key, Direction::Release)?;
        
        // Update state
        let mut keyboard_state = self.keyboard_state.write().await;
        keyboard_state.pressed_keys.remove(key);
        
        debug!("Key '{}' released", key);
        Ok(())
    }
    
    async fn handle_key_press(&mut self, key: &str, config: &InputConfig) -> Result<()> {
        let enigo_key = self.convert_key(key)?;
        
        self.enigo.key(enigo_key, Direction::Press)?;
        tokio::time::sleep(Duration::from_millis(config.key_repeat_delay)).await;
        self.enigo.key(enigo_key, Direction::Release)?;
        
        debug!("Key '{}' pressed and released", key);
        Ok(())
    }
    
    async fn handle_text_input(&mut self, text: &str) -> Result<()> {
        self.enigo.text(text)?;
        debug!("Text input: '{}'", text);
        Ok(())
    }
    
    async fn apply_modifiers(&mut self, modifiers: &[KeyModifier]) -> Result<()> {
        for modifier in modifiers {
            let key = match modifier {
                KeyModifier::Ctrl => Key::Control,
                KeyModifier::Alt => Key::Alt,
                KeyModifier::Shift => Key::Shift,
                KeyModifier::Meta | KeyModifier::Super => Key::Meta,
            };
            
            self.enigo.key(key, Direction::Press)?;
        }
        
        Ok(())
    }
    
    fn convert_mouse_button(&self, button: MouseButtonType) -> Button {
        match button {
            MouseButtonType::Left => Button::Left,
            MouseButtonType::Right => Button::Right,
            MouseButtonType::Middle => Button::Middle,
            MouseButtonType::X1 | MouseButtonType::X2 => Button::Left, // Fallback
        }
    }
    
    fn convert_key(&self, key_str: &str) -> Result<Key> {
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
            "enter" | "return" => Key::Return,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "escape" | "esc" => Key::Escape,
            
            // Arrow keys
            "up" | "arrowup" => Key::UpArrow,
            "down" | "arrowdown" => Key::DownArrow,
            "left" | "arrowleft" => Key::LeftArrow,
            "right" | "arrowright" => Key::RightArrow,
            
            // Modifier keys
            "shift" => Key::Shift,
            "ctrl" | "control" => Key::Control,
            "alt" => Key::Alt,
            "meta" | "cmd" | "super" => Key::Meta,
            
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
            
            // Other keys
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
    
    /// Update configuration
    pub async fn update_config(&self, new_config: InputConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated input configuration");
        Ok(())
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> InputConfig {
        self.config.read().await.clone()
    }
    
    /// Get input statistics
    pub async fn get_input_stats(&self) -> InputStats {
        let mouse_state = self.mouse_state.read().await;
        let keyboard_state = self.keyboard_state.read().await;
        
        InputStats {
            mouse_position: mouse_state.last_position,
            pressed_mouse_buttons: mouse_state.pressed_buttons.len(),
            pressed_keys: keyboard_state.pressed_keys.len(),
            is_dragging: mouse_state.drag_state.is_some(),
            active_modifiers: keyboard_state.active_modifiers.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InputStats {
    pub mouse_position: (i32, i32),
    pub pressed_mouse_buttons: usize,
    pub pressed_keys: usize,
    pub is_dragging: bool,
    pub active_modifiers: Vec<KeyModifier>,
}