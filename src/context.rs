use std::path::PathBuf;

pub struct Context {
    base_dir: PathBuf,
    repo: String,
}

impl Context {
    pub fn new(base_dir: Option<PathBuf>, repo: String) -> Self {
        let base_dir = base_dir.unwrap_or_else(|| {
            dirs::data_dir()
                .expect("could not determine data directory")
                .join("homeos")
        });
        Self { base_dir, repo }
    }

    pub fn repos_dir(&self) -> PathBuf {
        self.base_dir.join("repos")
    }

    pub fn repo_dir(&self) -> PathBuf {
        self.repos_dir().join(&self.repo)
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.repo_dir().join("packages")
    }

    pub fn config_path(&self) -> PathBuf {
        self.repo_dir().join("homeos.yml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.repo_dir().join("state.yml")
    }

    pub fn gitignore_path(&self) -> PathBuf {
        self.repo_dir().join(".gitignore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_repos_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.repos_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos"));
    }

    #[test]
    fn test_repo_dir_default() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.repo_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default"));
    }

    #[test]
    fn test_repo_dir_custom() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "my-repo".to_string());

        // Act
        let result = sut.repo_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/my-repo"));
    }

    #[test]
    fn test_packages_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.packages_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/packages"));
    }

    #[test]
    fn test_config_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.config_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/homeos.yml"));
    }

    #[test]
    fn test_state_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.state_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/state.yml"));
    }

    #[test]
    fn test_gitignore_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "default".to_string());

        // Act
        let result = sut.gitignore_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/repos/default/.gitignore"));
    }

    #[test]
    fn test_paths_with_custom_repo() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")), "work".to_string());

        // Act & Assert
        assert_eq!(sut.repo_dir(), Path::new("/tmp/test-homeos/repos/work"));
        assert_eq!(sut.packages_dir(), Path::new("/tmp/test-homeos/repos/work/packages"));
        assert_eq!(sut.config_path(), Path::new("/tmp/test-homeos/repos/work/homeos.yml"));
        assert_eq!(sut.state_path(), Path::new("/tmp/test-homeos/repos/work/state.yml"));
        assert_eq!(sut.gitignore_path(), Path::new("/tmp/test-homeos/repos/work/.gitignore"));
    }

    #[test]
    fn test_default_base_dir() {
        // Arrange
        let expected = dirs::data_dir().unwrap().join("homeos");

        // Act
        let sut = Context::new(None, "default".to_string());

        // Assert
        assert_eq!(sut.base_dir, expected);
    }
}
