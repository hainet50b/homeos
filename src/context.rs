use std::path::{Path, PathBuf};

pub struct Context {
    data_dir: PathBuf,
}

impl Context {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let data_dir = data_dir
            .or_else(|| std::env::var_os("HOMEOS_DATA_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                dirs::data_local_dir()
                    .expect("could not determine data directory")
                    .join("homeos")
            });
        Self { data_dir }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("homeos.yml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.data_dir.join("state.yml")
    }

    pub fn gitignore_path(&self) -> PathBuf {
        self.data_dir.join(".gitignore")
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const ENV_VAR: &str = "HOMEOS_DATA_DIR";

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    struct EnvVarGuard {
        previous: Option<std::ffi::OsString>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvVarGuard {
        fn capture() -> Self {
            let lock = env_lock();
            Self {
                previous: std::env::var_os(ENV_VAR),
                _lock: lock,
            }
        }

        fn set(value: &str) {
            unsafe {
                std::env::set_var(ENV_VAR, value);
            }
        }

        fn unset() {
            unsafe {
                std::env::remove_var(ENV_VAR);
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { std::env::set_var(ENV_VAR, value) },
                None => unsafe { std::env::remove_var(ENV_VAR) },
            }
        }
    }

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
    fn test_gitignore_path() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.gitignore_path();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/.gitignore"));
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
    fn test_plugins_dir() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.plugins_dir();

        // Assert
        assert_eq!(result, Path::new("/tmp/test-homeos/plugins"));
    }

    #[test]
    fn test_default_data_dir_when_env_var_unset() {
        // Arrange
        let _guard = EnvVarGuard::capture();
        EnvVarGuard::unset();
        let expected = dirs::data_local_dir().unwrap().join("homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), expected);
    }

    #[test]
    fn test_env_var_overrides_default() {
        // Arrange
        let _guard = EnvVarGuard::capture();
        EnvVarGuard::set("/tmp/env-homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/env-homeos"));
    }

    #[test]
    fn test_env_var_value_is_used_verbatim_without_homeos_segment() {
        // Arrange
        let _guard = EnvVarGuard::capture();
        EnvVarGuard::set("/tmp/custom-data");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/custom-data"));
    }

    #[test]
    fn test_explicit_arg_overrides_env_var() {
        // Arrange
        let _guard = EnvVarGuard::capture();
        EnvVarGuard::set("/tmp/env-homeos");

        // Act
        let sut = Context::new(Some(PathBuf::from("/tmp/explicit-homeos")));

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/explicit-homeos"));
    }
}
