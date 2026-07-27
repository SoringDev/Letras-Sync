use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub font_size: u32,
    pub font_family: String,
    pub font_color: String,
    pub background_color: String,
    pub projector_monitor: Option<u32>,
    pub cache_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 48,
            font_family: "Sans".to_string(),
            font_color: "#FFFF00".to_string(),
            background_color: "#000000".to_string(),
            projector_monitor: None,
            cache_path: String::new(),
        }
    }
}
