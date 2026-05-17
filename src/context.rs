use std::path::{Path, PathBuf};

pub struct Context {
    data_dir: PathBuf,
}

impl Context {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let data_dir = data_dir.unwrap_or_else(|| {
            dirs::data_local_dir()
                .expect("could not determine data directory")
                .join("homeos")
        });
        Self { data_dir }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("homeos.yml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.data_dir.join("state.yml")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }

    pub fn gitignore_path(&self) -> PathBuf {
        self.data_dir.join(".gitignore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_accessor() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.data_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos"));
    }

    #[test]
    fn test_packages_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.packages_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/packages"));
    }

    #[test]
    fn test_config_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.config_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/homeos.yml"));
    }

    #[test]
    fn test_state_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.state_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/state.yml"));
    }

    #[test]
    fn test_plugins_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.plugins_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/plugins"));
    }

    #[test]
    fn test_gitignore_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.gitignore_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/.gitignore"));
    }

    #[test]
    fn test_default_data_dir() {
        // Arrange
        let expected = dirs::data_local_dir().unwrap().join("homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), expected);
    }
}
