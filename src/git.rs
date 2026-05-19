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

pub fn init(target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["init", "--initial-branch=main", &target.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HomeosError::new(
            reasons::INTERNAL_ERROR,
            format!("git init failed: {}", stderr.trim()),
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

    #[test]
    fn test_init_creates_git_directory_in_empty_target() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("new-repo");
        std::fs::create_dir_all(&target).unwrap();

        // Act
        let result = init(&target);

        // Assert
        assert!(result.is_ok(), "init returned error: {:?}", result.err());
        assert!(target.join(".git").exists());
    }

    #[test]
    fn test_init_succeeds_in_directory_with_files() {
        // Arrange — git init is idempotent and should work even when the
        // target already contains regular files (the homeos scaffold case).
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("repo-with-files");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("homeos.yml"), "packages: {}\n").unwrap();
        std::fs::write(target.join(".gitignore"), "state.yml\n").unwrap();

        // Act
        let result = init(&target);

        // Assert
        assert!(result.is_ok());
        assert!(target.join(".git").exists());
        // Pre-existing files are untouched.
        assert!(target.join("homeos.yml").exists());
        assert!(target.join(".gitignore").exists());
    }

    #[test]
    fn test_init_sets_initial_branch_to_main() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("new-repo");
        std::fs::create_dir_all(&target).unwrap();

        // Act
        init(&target).unwrap();

        // Assert — the freshly initialized repo's symbolic HEAD must point at
        // refs/heads/main regardless of the user's `init.defaultBranch` config.
        let output = Command::new("git")
            .args([
                "-C",
                &target.to_string_lossy(),
                "symbolic-ref",
                "--short",
                "HEAD",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "symbolic-ref failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let head = String::from_utf8(output.stdout).unwrap();
        assert_eq!(head.trim(), "main");
    }

    #[test]
    fn test_init_is_idempotent_on_existing_repo() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("repo");
        std::fs::create_dir_all(&target).unwrap();
        init(&target).unwrap();

        // Act — running init a second time on an existing repo is a no-op
        // that still reports success (matching `git init`'s native behavior).
        let result = init(&target);

        // Assert
        assert!(result.is_ok());
        assert!(target.join(".git").exists());
    }
}
