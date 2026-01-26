use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigJournal {
    pub files: Vec<PathBuf>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigStaging {
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub journal: ConfigJournal,
    pub staging: ConfigStaging,
}

impl Config {
    pub fn load_from_file(path: &Path) -> Result<(PathBuf, Self)> {
        let base_dir = path.parent().map(ToOwned::to_owned).unwrap_or_default();

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok((base_dir, config))
    }

    pub fn find_and_load() -> Result<Option<(PathBuf, Self)>> {
        let config_locations = [
            Path::new("beancount-staging.toml"),
            Path::new(".beancount-staging.toml"),
        ];

        for location in &config_locations {
            if location.exists() {
                return Self::load_from_file(location).map(Some);
            }
        }

        Ok(None)
    }
}
