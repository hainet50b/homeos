use crate::output::OutputFormat;
use std::path::{Path, PathBuf};

pub struct Context {
    data_dir: PathBuf,
    output_format: OutputFormat,
    yes: bool,
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
        Self {
            data_dir,
            output_format: OutputFormat::default(),
            yes: false,
        }
    }

    pub fn try_new() -> Option<Self> {
        let data_dir = std::env::var_os("HOMEOS_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::data_local_dir().map(|d| d.join("homeos")))?;
        Some(Self {
            data_dir,
            output_format: OutputFormat::default(),
            yes: false,
        })
    }

    pub fn with_output_format(mut self, output_format: OutputFormat) -> Self {
        self.output_format = output_format;
        self
    }

    pub fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    #[allow(dead_code)]
    pub fn output_format(&self) -> OutputFormat {
        self.output_format
    }

    pub fn yes(&self) -> bool {
        self.yes
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
    use crate::env_test::EnvVarGuard;

    const ENV_VAR: &str = "HOMEOS_DATA_DIR";

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
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();
        let expected = dirs::data_local_dir().unwrap().join("homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), expected);
    }

    #[test]
    fn test_env_var_overrides_default() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("/tmp/env-homeos");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/env-homeos"));
    }

    #[test]
    fn test_env_var_value_is_used_verbatim_without_homeos_segment() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("/tmp/custom-data");

        // Act
        let sut = Context::new(None);

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/custom-data"));
    }

    #[test]
    fn test_explicit_arg_overrides_env_var() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("/tmp/env-homeos");

        // Act
        let sut = Context::new(Some(PathBuf::from("/tmp/explicit-homeos")));

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/explicit-homeos"));
    }

    #[test]
    fn test_try_new_returns_some_when_env_var_set() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("/tmp/try-new-homeos");

        // Act
        let sut = Context::try_new();

        // Assert
        let ctx = sut.expect("try_new should return Some when env var is set");
        assert_eq!(ctx.data_dir(), Path::new("/tmp/try-new-homeos"));
    }

    #[test]
    fn test_default_output_format_is_text() {
        // Arrange
        let sut = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let result = sut.output_format();

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_with_output_format_sets_format() {
        // Arrange
        let ctx = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let sut = ctx.with_output_format(OutputFormat::Json);

        // Assert
        assert_eq!(sut.output_format(), OutputFormat::Json);
    }

    #[test]
    fn test_with_output_format_preserves_data_dir() {
        // Arrange
        let ctx = Context::new(Some(PathBuf::from("/tmp/test-homeos")));

        // Act
        let sut = ctx.with_output_format(OutputFormat::Json);

        // Assert
        assert_eq!(sut.data_dir(), Path::new("/tmp/test-homeos"));
    }

    #[test]
    fn test_try_new_uses_data_local_dir_when_env_var_unset() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();
        let expected = dirs::data_local_dir().unwrap().join("homeos");

        // Act
        let sut = Context::try_new();

        // Assert
        let ctx = sut.expect("try_new should return Some on platforms with data_local_dir");
        assert_eq!(ctx.data_dir(), expected);
    }
}
