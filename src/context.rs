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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture(base_dir: Option<PathBuf>) -> Context {
        Context::new(base_dir)
    }

    #[test]
    fn test_custom_base_dir() {
        // Arrange
        let base = PathBuf::from("/tmp/test-homeos");

        // Act
        let sut = fixture(Some(base));

        // Assert
        assert_eq!(sut.repos_dir(), Path::new("/tmp/test-homeos/repos"));
        assert_eq!(
            sut.default_repo_dir(),
            Path::new("/tmp/test-homeos/repos/default")
        );
        assert_eq!(
            sut.packages_dir(),
            Path::new("/tmp/test-homeos/repos/default/packages")
        );
        assert_eq!(
            sut.config_path(),
            Path::new("/tmp/test-homeos/repos/default/homeos.yml")
        );
    }

    #[test]
    fn test_default_base_dir() {
        // Arrange
        let expected = dirs::data_dir().unwrap().join("homeos");

        // Act
        let sut = fixture(None);

        // Assert
        assert_eq!(sut.base_dir, expected);
    }
}
