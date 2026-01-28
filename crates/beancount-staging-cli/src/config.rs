use anyhow::{Context, Result};
use beancount_staging::reconcile::StagingSource;
use serde::Deserialize;
use std::path::PathBuf;

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
    pub fn load_from_file(path: &std::path::Path) -> Result<(PathBuf, Self)> {
        let base_dir = path.parent().map(ToOwned::to_owned).unwrap_or_default();

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok((base_dir, config))
    }

    pub fn find_and_load() -> Result<Option<(PathBuf, Self)>> {
        let config_locations = [
            std::path::Path::new("beancount-staging.toml"),
            std::path::Path::new(".beancount-staging.toml"),
        ];

        for location in &config_locations {
            if location.exists() {
                return Self::load_from_file(location).map(Some);
            }
        }

        Ok(None)
    }
}
