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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
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
        let config: Config = yaml_serde::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let contents = yaml_serde::to_string(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn fixture_file(content: &str) -> NamedTempFile {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        tmp
    }

    #[test]
    fn test_parse_full_config() {
        // Arrange
        let yaml = r#"
packages:
  neovim:
    actions_overrides:
      update: install
    enabled: false
  ripgrep:
    enabled: true
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.packages.len(), 2);
        let neovim = &sut.packages["neovim"];
        assert_eq!(neovim.actions_overrides["update"], "install");
        assert!(!neovim.enabled);
        let ripgrep = &sut.packages["ripgrep"];
        assert!(ripgrep.actions_overrides.is_empty());
        assert!(ripgrep.enabled);
    }

    #[test]
    fn test_parse_empty_packages() {
        // Arrange
        let yaml = "packages: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages.is_empty());
    }

    #[test]
    fn test_parse_minimal_package() {
        // Arrange
        let yaml = "packages:\n  git: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let git = &sut.packages["git"];
        assert!(git.enabled);
        assert!(git.actions_overrides.is_empty());
    }

    #[test]
    fn test_defaults_on_missing_fields() {
        // Arrange
        let yaml = "packages:\n  fish: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let fish = &sut.packages["fish"];
        assert!(fish.enabled);
        assert!(fish.actions_overrides.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        // Arrange
        let tmp = fixture_file(
            "packages:\n  neovim:\n    actions_overrides:\n      update: install\n    enabled: false\n",
        );

        // Act
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.packages.len(), 1);
        assert!(!sut.packages["neovim"].enabled);
    }

    #[test]
    fn test_save_and_reload() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "starship".to_string(),
            PackageConfig {
                actions_overrides: BTreeMap::from([("update".to_string(), "install".to_string())]),
                enabled: true,
                depends_on: Vec::new(),
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(config, sut);
    }

    #[test]
    fn test_serialize_skips_defaults() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    actions_overrides: BTreeMap::new(),
                    enabled: true,
                    depends_on: Vec::new(),
                },
            )]),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("actions_overrides"));
        assert!(!sut.contains("enabled"));
    }

    #[test]
    fn test_parse_depends_on() {
        // Arrange
        let yaml = r#"
packages:
  neovim:
    depends_on:
      - git
      - curl
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let neovim = &sut.packages["neovim"];
        assert_eq!(neovim.depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_depends_on_defaults_to_empty() {
        // Arrange
        let yaml = "packages:\n  git: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages["git"].depends_on.is_empty());
    }

    #[test]
    fn test_serialize_skips_empty_depends_on() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    depends_on: Vec::new(),
                    ..Default::default()
                },
            )]),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("depends_on"));
    }

    #[test]
    fn test_serialize_includes_nonempty_depends_on() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig {
                    depends_on: vec!["git".to_string(), "curl".to_string()],
                    ..Default::default()
                },
            )]),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("depends_on"));
        assert!(sut.contains("git"));
        assert!(sut.contains("curl"));
    }

    #[test]
    fn test_save_and_reload_with_depends_on() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "neovim".to_string(),
            PackageConfig {
                depends_on: vec!["git".to_string(), "curl".to_string()],
                ..Default::default()
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_load_nonexistent_file() {
        // Arrange
        let path = Path::new("/nonexistent/homeos.yml");

        // Act
        let result = Config::load(path);

        // Assert
        assert!(result.is_err());
    }
}
