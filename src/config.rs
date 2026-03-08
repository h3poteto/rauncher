use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use toml;

use crate::error;

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub hotkey: Hotkey,
    pub custom_search: Vec<CustomSearch>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Hotkey {
    pub key: u8,
    pub modifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CustomSearch {
    pub name: String,
    pub exec: String,
    pub icon_path: Option<String>,
    pub icon_name: Option<String>,
    pub default_search: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: Hotkey {
                key: 65,
                modifier: "ctrl".to_string(),
            },
            custom_search: vec![CustomSearch {
                name: "Google".to_string(),
                exec: "https://www.google.com/search?q=%q".to_string(),
                icon_path: None,
                icon_name: Some("web-browser".to_string()),
                default_search: true,
            }],
        }
    }
}

pub fn parse_config(path: &PathBuf) -> Result<Config, error::Error> {
    let body = fs::read_to_string(path)?;
    let config: Config = toml::from_str(body.as_str()).unwrap_or_default();
    Ok(config)
}

pub fn write_default_config(dir: &PathBuf, file: &PathBuf) -> Result<Config, error::Error> {
    let c = Config::default();
    let body = toml::to_string(&c)?;
    if !fs::exists(dir)? {
        let _ = fs::create_dir(dir)?;
    }

    let _ = fs::write(file, body)?;
    Ok(c)
}
