use crate::error::{HomeosError, reasons};
use std::path::Path;
use std::process::Command;

pub fn clone(url: &str, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["clone", url, &target.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HomeosError::new(
            reasons::GIT_CLONE_FAILED,
            format!("git clone failed: {}", stderr.trim()),
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_local_git_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        Command::new("git")
            .args(["init", &dir.to_string_lossy()])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
    }

    #[test]
    fn test_clone_succeeds_for_valid_local_repo() {
        // Arrange
        let source = TempDir::new().unwrap();
        create_local_git_repo(source.path());
        let target_parent = TempDir::new().unwrap();
        let target = target_parent.path().join("cloned");

        // Act
        let result = clone(&source.path().to_string_lossy(), &target);

        // Assert
        assert!(result.is_ok());
        assert!(target.exists());
        assert!(target.join(".git").exists());
    }

    #[test]
    fn test_clone_fails_for_invalid_url() {
        // Arrange
        let target_parent = TempDir::new().unwrap();
        let target = target_parent.path().join("cloned");

        // Act
        let result = clone("not-a-valid-url", &target);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.starts_with("git clone failed:"),
            "unexpected error: {}",
            err
        );
    }
}
