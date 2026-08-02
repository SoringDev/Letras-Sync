use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionConfig {
    pub font_family: String,
    pub font_size: u32,
    pub font_weight: u32,
    pub letter_spacing: f64,
    pub line_height_multiplier: f64,
    pub dynamic_font_scaling: bool,
    pub min_font_size: u32,
    pub max_font_multiplier: f64,
    pub margin_horizontal: u32,
    pub margin_vertical: u32,
    pub horizontal_alignment: String,
    pub vertical_alignment: String,
    pub font_color: String,
    pub background_color: String,
    pub shadow_enabled: bool,
    pub shadow_color: String,
    pub shadow_offset_x: i32,
    pub shadow_offset_y: i32,
    pub fade_duration_ms: u32,
    pub fade_animation_enabled: bool,
}

impl Default for ProjectionConfig {
    fn default() -> Self {
        Self {
            font_family: "Inter".to_string(),
            font_size: 150,               // Grande para ocupar a maior parte da tela
            font_weight: 600,             // ExtraBold melhora leitura à distância
            letter_spacing: -0.3,         // Leve redução evita espaçamento excessivo
            line_height_multiplier: 0.95, // Uma única linha não precisa de espaçamento extra
            dynamic_font_scaling: true,
            min_font_size: 32,
            max_font_multiplier: 1.5,
            margin_horizontal: 70,        // Evita que textos longos encostem nas bordas
            margin_vertical: 30,
            horizontal_alignment: "center".to_string(),
            vertical_alignment: "center".to_string(),
            font_color: "#FFFFFF".to_string(), // Branco puro oferece maior contraste
            background_color: "#000000".to_string(),
            shadow_enabled: false,
            shadow_color: "#000000".to_string(),
            shadow_offset_x: 4,
            shadow_offset_y: 4,
            fade_duration_ms: 150,
            fade_animation_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_inter_font() {
        let config = ProjectionConfig::default();
        assert_eq!(config.font_family, "Inter");
        assert_eq!(config.font_size, 150);
        assert!(!config.shadow_enabled);
        assert!(config.dynamic_font_scaling);
        assert_eq!(config.min_font_size, 32);
        assert_eq!(config.max_font_multiplier, 1.5);
    }
}
