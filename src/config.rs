use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub packages: BTreeMap<String, PackageConfig>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PackageConfig {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions_overrides: BTreeMap<String, String>,
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let contents = serde_yaml::to_string(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
packages:
  neovim:
    actions_overrides:
      update: install
    enabled: false
  ripgrep:
    enabled: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.packages.len(), 2);

        let neovim = &config.packages["neovim"];
        assert_eq!(neovim.actions_overrides["update"], "install");
        assert!(!neovim.enabled);

        let ripgrep = &config.packages["ripgrep"];
        assert!(ripgrep.actions_overrides.is_empty());
        assert!(ripgrep.enabled);
    }

    #[test]
    fn test_parse_empty_packages() {
        let yaml = "packages: {}\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_parse_minimal_package() {
        let yaml = r#"
packages:
  git: {}
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let git = &config.packages["git"];
        assert!(git.enabled);
        assert!(git.actions_overrides.is_empty());
    }

    #[test]
    fn test_defaults_on_missing_fields() {
        let yaml = r#"
packages:
  fish: {}
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let fish = &config.packages["fish"];
        assert!(fish.enabled);
        assert!(fish.actions_overrides.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(
            tmp,
            "packages:\n  neovim:\n    actions_overrides:\n      update: install\n    enabled: false\n"
        )
        .unwrap();

        let config = Config::load(tmp.path()).unwrap();
        assert_eq!(config.packages.len(), 1);
        assert!(!config.packages["neovim"].enabled);
    }

    #[test]
    fn test_save_and_reload() {
        let mut config = Config::default();
        config.packages.insert(
            "starship".to_string(),
            PackageConfig {
                actions_overrides: BTreeMap::from([("update".to_string(), "install".to_string())]),
                enabled: true,
            },
        );

        let tmp = NamedTempFile::new().unwrap();
        config.save(tmp.path()).unwrap();

        let reloaded = Config::load(tmp.path()).unwrap();
        assert_eq!(config, reloaded);
    }

    #[test]
    fn test_serialize_skips_defaults() {
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    actions_overrides: BTreeMap::new(),
                    enabled: true,
                },
            )]),
        };
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(!yaml.contains("actions_overrides"));
        assert!(!yaml.contains("enabled"));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = Config::load(Path::new("/nonexistent/homeos.yml"));
        assert!(result.is_err());
    }
}
