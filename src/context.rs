use std::path::PathBuf;

pub struct Context {
    base_dir: PathBuf,
}

impl Context {
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        let base_dir = base_dir.unwrap_or_else(|| {
            dirs::data_dir()
                .expect("could not determine data directory")
                .join("homeos")
        });
        Self { base_dir }
    }

    pub fn repos_dir(&self) -> PathBuf {
        self.base_dir.join("repos")
    }

    pub fn default_repo_dir(&self) -> PathBuf {
        self.repos_dir().join("default")
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.default_repo_dir().join("packages")
    }

    pub fn config_path(&self) -> PathBuf {
        self.default_repo_dir().join("homeos.yml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.default_repo_dir().join("state.yml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_repos_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.repos_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos"));
    }

    #[test]
    fn test_default_repo_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.default_repo_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default"));
    }

    #[test]
    fn test_packages_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.packages_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/packages"));
    }

    #[test]
    fn test_config_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.config_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/homeos.yml"));
    }

    #[test]
    fn test_state_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.state_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/state.yml"));
    }

    #[test]
    fn test_default_base_dir() {
        // Arrange
        let expected = dirs::data_dir().unwrap().join("homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.base_dir, expected);
    }
}
