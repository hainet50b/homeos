use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    #[serde(default)]
    pub installed: Vec<String>,
}

impl State {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let state: State = yaml_serde::from_str(&contents)?;
        Ok(state)
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
    fn test_parse_installed_packages() {
        // Arrange
        let yaml = "installed:\n  - neovim\n  - zed\n";

        // Act
        let sut: State = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.installed, vec!["neovim", "zed"]);
    }

    #[test]
    fn test_parse_empty_installed() {
        // Arrange
        let yaml = "installed: []\n";

        // Act
        let sut: State = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.installed.is_empty());
    }

    #[test]
    fn test_defaults_on_missing_fields() {
        // Arrange
        let yaml = "{}\n";

        // Act
        let sut: State = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.installed.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        // Arrange
        let tmp = fixture_file("installed:\n  - neovim\n  - zed\n");

        // Act
        let sut = State::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.installed, vec!["neovim", "zed"]);
    }

    #[test]
    fn test_save_and_reload() {
        // Arrange
        let state = State {
            installed: vec!["neovim".to_string(), "zed".to_string()],
        };
        let tmp = NamedTempFile::new().unwrap();

        // Act
        state.save(tmp.path()).unwrap();
        let sut = State::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(state, sut);
    }

    #[test]
    fn test_load_nonexistent_file() {
        // Arrange
        let path = Path::new("/nonexistent/state.yml");

        // Act
        let result = State::load(path);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_default_state_has_empty_installed() {
        // Arrange / Act
        let sut = State::default();

        // Assert
        assert!(sut.installed.is_empty());
    }
}
