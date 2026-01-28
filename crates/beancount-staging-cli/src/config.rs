use anyhow::{Context, Result};
use beancount_staging::reconcile::StagingSource;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigJournal {
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawConfigStaging")]
pub struct ConfigStaging(pub StagingSource);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfigStaging {
    #[serde(default)]
    files: Vec<PathBuf>,
    #[serde(default)]
    command: Vec<String>,
}

impl TryFrom<RawConfigStaging> for ConfigStaging {
    type Error = String;

    fn try_from(raw: RawConfigStaging) -> Result<Self, Self::Error> {
        match (raw.files.is_empty(), raw.command.is_empty()) {
            (false, true) => Ok(ConfigStaging(StagingSource::Files(raw.files))),
            (true, false) => Ok(ConfigStaging(StagingSource::Command {
                command: raw.command,
                cwd: PathBuf::from("."),
            })),
            (true, true) => {
                Err("staging section must have either 'files' or 'command' specified".to_string())
            }
            (false, false) => {
                Err("staging section cannot have both 'files' and 'command' specified".to_string())
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub journal: ConfigJournal,
    pub staging: ConfigStaging,
}

impl Config {
    fn find_config_in_dir(dir: &Path) -> Option<PathBuf> {
        let config_locations = [
            dir.join("beancount-staging.toml"),
            dir.join(".beancount-staging.toml"),
        ];

        config_locations.into_iter().find(|p| p.exists())
    }

    pub fn load_from_file(path: &Path) -> Result<(PathBuf, Self)> {
        // If path is a directory, look for config file in that directory
        let (config_path, base_dir) = if path.is_dir() {
            let config_path = Self::find_config_in_dir(path).ok_or_else(|| {
                anyhow::anyhow!(
                    "No config file found in directory: {} (tried: beancount-staging.toml, .beancount-staging.toml)",
                    path.display()
                )
            })?;

            (config_path, path.to_path_buf())
        } else {
            let base_dir = path.parent().map(ToOwned::to_owned).unwrap_or_default();
            (path.to_path_buf(), base_dir)
        };

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        Ok((base_dir, config))
    }

    pub fn find_and_load() -> Result<Option<(PathBuf, Self)>> {
        if let Some(config_path) = Self::find_config_in_dir(Path::new(".")) {
            return Self::load_from_file(&config_path).map(Some);
        }

        Ok(None)
    }
}
