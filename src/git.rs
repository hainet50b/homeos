use crate::error::{HomeosError, reasons};
use std::path::Path;
use std::process::Command;

pub fn clone(url: &str, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    clone_with("git", url, target)
}

fn clone_with(program: &str, url: &str, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(program)
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.eol=lf",
            "clone",
            url,
            &target.to_string_lossy(),
        ])
        .output()
        .map_err(spawn_error)?;

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
    init_with("git", target)
}

fn init_with(program: &str, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(program)
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.eol=lf",
            "init",
            "--initial-branch=main",
            &target.to_string_lossy(),
        ])
        .output()
        .map_err(spawn_error)?;

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

fn spawn_error(e: std::io::Error) -> Box<dyn std::error::Error> {
    if e.kind() == std::io::ErrorKind::NotFound {
        HomeosError::new(
            reasons::GIT_NOT_FOUND,
            "git not found on PATH. homeos requires Git 2.28+.",
        )
        .into()
    } else {
        e.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_test::EnvVarGuard;
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
    fn test_clone_preserves_lf_when_global_config_requests_autocrlf() {
        // Arrange — create a source repo with an LF-only text file committed,
        // and force `* text` classification so git is unambiguous about
        // treating it as text. Then point GIT_CONFIG_GLOBAL at a config file
        // that sets `core.autocrlf=true`, simulating a Git for Windows-style
        // default that would rewrite LF to CRLF on checkout if our
        // `-c core.autocrlf=false` override were missing.
        let source = TempDir::new().unwrap();
        std::fs::create_dir_all(source.path()).unwrap();
        let source_path = source.path().to_string_lossy().to_string();
        let run_git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-C", &source_path])
                .args(args)
                .output()
                .unwrap();
            assert!(
                status.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&status.stderr)
            );
        };
        run_git(&["init", "--initial-branch=main"]);
        run_git(&["config", "user.name", "test"]);
        run_git(&["config", "user.email", "test@test"]);
        std::fs::write(source.path().join(".gitattributes"), "* text\n").unwrap();
        std::fs::write(source.path().join("script.sh"), "echo hi\n").unwrap();
        run_git(&["add", "-A"]);
        run_git(&["commit", "-m", "seed"]);

        let global_config_home = TempDir::new().unwrap();
        let global_config = global_config_home.path().join("gitconfig");
        std::fs::write(&global_config, "[core]\n\tautocrlf = true\n").unwrap();
        let guard = EnvVarGuard::capture("GIT_CONFIG_GLOBAL");
        guard.set(&global_config.to_string_lossy());

        let target_parent = TempDir::new().unwrap();
        let target = target_parent.path().join("cloned");

        // Act
        let result = clone(&source_path, &target);

        // Assert
        assert!(result.is_ok(), "clone returned error: {:?}", result.err());
        let cloned = std::fs::read(target.join("script.sh")).unwrap();
        assert_eq!(
            cloned, b"echo hi\n",
            "LF was rewritten despite the -c core.autocrlf=false override; got bytes {:?}",
            cloned
        );
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
    fn test_clone_with_missing_program_yields_git_not_found() {
        // Arrange
        let target_parent = TempDir::new().unwrap();
        let target = target_parent.path().join("cloned");

        // Act
        let result = clone_with("homeos-test-no-such-git", "some-url", &target);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err.downcast_ref::<HomeosError>().unwrap();
        assert_eq!(homeos_err.reason, reasons::GIT_NOT_FOUND);
        assert_eq!(
            homeos_err.message,
            "git not found on PATH. homeos requires Git 2.28+."
        );
    }

    #[test]
    fn test_init_with_missing_program_yields_git_not_found() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("new-repo");
        std::fs::create_dir_all(&target).unwrap();

        // Act
        let result = init_with("homeos-test-no-such-git", &target);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err.downcast_ref::<HomeosError>().unwrap();
        assert_eq!(homeos_err.reason, reasons::GIT_NOT_FOUND);
        assert_eq!(
            homeos_err.message,
            "git not found on PATH. homeos requires Git 2.28+."
        );
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
