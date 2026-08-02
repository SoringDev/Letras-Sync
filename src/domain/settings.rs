use serde::{Deserialize, Serialize};

use crate::domain::projection_config::ProjectionConfig;

fn default_volume() -> u32 {
    100
}

fn default_autoplay() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub font_size: u32,
    pub font_family: String,
    pub font_color: String,
    pub background_color: String,
    pub projector_monitor: Option<u32>,
    pub cache_path: String,
    #[serde(default)]
    pub projection: ProjectionConfig,
    #[serde(default = "default_volume")]
    pub volume: u32,
    #[serde(default = "default_autoplay")]
    pub autoplay: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 48,
            font_family: "Inter".to_string(),
            font_color: "#FFFF00".to_string(),
            background_color: "#000000".to_string(),
            projector_monitor: None,
            cache_path: String::new(),
            projection: ProjectionConfig::default(),
            volume: default_volume(),
            autoplay: default_autoplay(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_round_trips_through_toml() {
        let settings = Settings {
            font_size: 48,
            font_family: "Inter".to_string(),
            font_color: "#FFFF00".to_string(),
            background_color: "#000000".to_string(),
            projector_monitor: None,
            cache_path: "/tmp/cache".to_string(),
            projection: ProjectionConfig::default(),
            volume: 73,
            autoplay: false,
        };

        let encoded = toml::to_string(&settings).expect("serializar settings");
        let decoded: Settings = toml::from_str(&encoded).expect("desserializar settings");

        assert_eq!(decoded, settings);
    }
}
