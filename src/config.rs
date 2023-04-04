use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::io::Write;

#[derive(Debug)]
pub enum ConfigError {
    FileError(std::io::Error),
    YamlError(serde_yaml::Error)
}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        ConfigError::FileError(value)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(value: serde_yaml::Error) -> Self {
        ConfigError::YamlError(value)
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileError(e) => write!(f, "Error when attempting to read config file: {}", e),
            Self::YamlError(e) => write!(f, "Error when attempting to parse config file: {}", e)
        }
    }
}

impl std::error::Error for ConfigError {

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server_address: String,
    pub project_directory: PathBuf
}

pub fn read_config_file(filepath: &Path) -> Result<Config, ConfigError> {
    let yaml_str = std::fs::read_to_string(filepath)?;

    return Ok(serde_yaml::from_str::<Config>(&yaml_str)?);
}

#[allow(dead_code)]
pub fn write_config_to_file(config: &Config, filepath: &Path) -> Result<(), ConfigError> {
    let mut handle = std::fs::File::create(filepath)?;
    let yaml_str = serde_yaml::to_string(&config)?;
    handle.write(yaml_str.as_bytes())?;

    Ok(())
}