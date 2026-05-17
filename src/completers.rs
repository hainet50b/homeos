use crate::config::Config;
use crate::context::Context;
use clap_complete::CompletionCandidate;
use std::ffi::OsStr;

pub fn package_completer(_current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(ctx) = Context::try_new() else {
        return Vec::new();
    };
    let Ok(config) = Config::load(&ctx.config_path()) else {
        return Vec::new();
    };
    config
        .packages
        .keys()
        .map(|k| CompletionCandidate::new(k.as_str()))
        .collect()
}

pub fn plugin_completer(_current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(ctx) = Context::try_new() else {
        return Vec::new();
    };
    let Ok(config) = Config::load(&ctx.config_path()) else {
        return Vec::new();
    };
    config
        .plugins
        .keys()
        .map(|k| CompletionCandidate::new(k.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_test::EnvVarGuard;
    use std::ffi::OsStr;
    use std::fs;
    use tempfile::TempDir;

    const ENV_VAR: &str = "HOMEOS_DATA_DIR";

    fn write_config(dir: &std::path::Path, contents: &str) {
        fs::write(dir.join("homeos.yml"), contents).unwrap();
    }

    fn candidate_values(candidates: &[CompletionCandidate]) -> Vec<String> {
        candidates
            .iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn test_package_completer_returns_all_package_names() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(
            tmp.path(),
            "packages:\n  neovim: {}\n  claude:\n    depends_on: [bubblewrap]\n  bubblewrap: {}\n",
        );
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = package_completer(OsStr::new(""));

        // Assert
        let mut values = candidate_values(&candidates);
        values.sort();
        assert_eq!(values, vec!["bubblewrap", "claude", "neovim"]);
    }

    #[test]
    fn test_package_completer_returns_empty_when_config_missing() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = package_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_package_completer_returns_empty_when_config_malformed() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), "not: valid: yaml: at: all:\n  - [bad");
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = package_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_package_completer_returns_empty_when_no_packages() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), "packages: {}\n");
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = package_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_plugin_completer_returns_all_plugin_names() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(
            tmp.path(),
            "packages: {}\nplugins:\n  dnf:\n    url: https://example.com/dnf\n  npm:\n    url: https://example.com/npm\n",
        );
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = plugin_completer(OsStr::new(""));

        // Assert
        let mut values = candidate_values(&candidates);
        values.sort();
        assert_eq!(values, vec!["dnf", "npm"]);
    }

    #[test]
    fn test_plugin_completer_returns_empty_when_config_missing() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = plugin_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_plugin_completer_returns_empty_when_no_plugins() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), "packages:\n  git: {}\n");
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = plugin_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_plugin_completer_returns_empty_when_config_malformed() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        write_config(tmp.path(), "not: valid: yaml: at: all:\n  - [bad");
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set(tmp.path().to_str().unwrap());

        // Act
        let candidates = plugin_completer(OsStr::new(""));

        // Assert
        assert!(candidates.is_empty());
    }
}
