use serde::Deserialize;
use std::fs;

use crate::constants::DOWN_SCALE;

#[derive(Debug, Clone, Deserialize)]
struct BodyConfigRaw {
    pub name: String,
    pub radius: f32,
    pub mass: f32,
    pub distance: f32,
    pub velocity: f32,
    pub color: [f32; 3],
    #[serde(default = "default_render_scale")]
    pub render_scale: f32,
    #[serde(default)]
    pub relative_to: Option<String>,
}

fn default_render_scale() -> f32 { 1.0 }

#[derive(Debug, Clone)]
pub struct BodyConfig {
    pub name: String,
    pub radius: f32,
    pub mass: f32,
    pub distance: f32,
    pub velocity: f32,
    pub color: [f32; 3],
    pub relative_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bodies: Vec<BodyConfig>,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let data = fs::read_to_string(path)?;
        let raws: Vec<BodyConfigRaw> = serde_json::from_str(&data)?;
        let bodies = raws.into_iter().map(|raw| BodyConfig {
            name: raw.name,
            radius: (raw.radius / DOWN_SCALE) * raw.render_scale,
            mass: raw.mass / DOWN_SCALE,
            distance: raw.distance / DOWN_SCALE,
            velocity: raw.velocity / DOWN_SCALE,
            color: raw.color,
            relative_to: raw.relative_to,
        }).collect();
        Ok(Self { bodies })
    }
}
