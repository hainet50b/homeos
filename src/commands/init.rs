use crate::config::Config;
use crate::context::Context;
use std::fs;
use std::process::Command;

pub fn run(ctx: &Context, url: Option<&str>, strip_git: bool) -> Result<(), Box<dyn std::error::Error>> {
    let repo_dir = ctx.repo_dir();
    let config_path = ctx.config_path();

    if config_path.exists() {
        println!("Already initialized at {}", repo_dir.display());
        return Ok(());
    }

    if let Some(url) = url {
        // Clone mode: clone remote repository as the default repo
        let repos_dir = ctx.repos_dir();
        fs::create_dir_all(&repos_dir)?;

        let output = Command::new("git")
            .args(["clone", url, &repo_dir.to_string_lossy()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git clone failed: {}", stderr.trim()).into());
        }

        if strip_git {
            let git_dir = repo_dir.join(".git");
            if git_dir.exists() {
                fs::remove_dir_all(&git_dir)?;
            }
        }

        println!("Initialized homeos at {} (cloned from {})", repo_dir.display(), url);
    } else {
        // Scaffold mode: create empty structure
        let packages_dir = ctx.packages_dir();
        fs::create_dir_all(&packages_dir)?;

        let config = Config::default();
        config.save(&config_path)?;

        let gitignore_path = ctx.gitignore_path();
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "state.yml\n")?;
        }

        println!("Initialized homeos at {}", repo_dir.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());
        (tmp, ctx)
    }

    fn create_local_git_repo(dir: &std::path::Path) {
        fs::create_dir_all(dir).unwrap();
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
    fn test_init_creates_structure() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        assert!(ctx.repo_dir().exists());
        assert!(ctx.packages_dir().exists());
        assert!(ctx.config_path().exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_init_idempotent() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.packages.insert(
            "test".to_string(),
            crate::config::PackageConfig::default(),
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages.len(), 1);
    }

    #[test]
    fn test_init_directory_paths() {
        // Arrange
        let (tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let base = tmp.path();
        assert!(base.join("repos/default/packages").exists());
        assert!(base.join("repos/default/homeos.yml").exists());
    }

    #[test]
    fn test_init_creates_gitignore_excluding_state_yml() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let gitignore_path = ctx.gitignore_path();
        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "state.yml\n");
    }

    #[test]
    fn test_init_idempotent_preserves_gitignore() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();
        fs::write(ctx.gitignore_path(), "state.yml\ncustom\n").unwrap();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let content = fs::read_to_string(ctx.gitignore_path()).unwrap();
        assert_eq!(content, "state.yml\ncustom\n");
    }

    #[test]
    fn test_init_with_url_clones_repo() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());
        // Add a homeos.yml to the source repo
        fs::write(source_dir.path().join("homeos.yml"), "packages: {}\n").unwrap();
        Command::new("git")
            .args([
                "-C",
                &source_dir.path().to_string_lossy(),
                "add",
                "homeos.yml",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &source_dir.path().to_string_lossy(),
                "commit",
                "-m",
                "add config",
            ])
            .output()
            .unwrap();

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        assert!(ctx.repo_dir().exists());
        assert!(ctx.config_path().exists());
        assert!(ctx.repo_dir().join(".git").exists());
    }

    #[test]
    fn test_init_with_url_skips_if_already_initialized() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();

        // Act
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert — original scaffold preserved, not replaced by clone
        assert!(ctx.packages_dir().exists());
    }

    #[test]
    fn test_init_with_url_invalid_url() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        let result = run(&ctx, Some("not-a-valid-url"), false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.starts_with("git clone failed:"));
    }

    #[test]
    fn test_init_with_url_creates_repos_dir() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());
        assert!(!ctx.repos_dir().exists());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        assert!(ctx.repos_dir().exists());
        assert!(ctx.repo_dir().exists());
    }

    fn create_source_repo_with_config(source_dir: &std::path::Path) {
        create_local_git_repo(source_dir);
        fs::write(source_dir.join("homeos.yml"), "packages: {}\n").unwrap();
        Command::new("git")
            .args(["-C", &source_dir.to_string_lossy(), "add", "homeos.yml"])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", &source_dir.to_string_lossy(), "commit", "-m", "add config"])
            .output()
            .unwrap();
    }

    #[test]
    fn test_init_strip_git_removes_git_directory() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), true).unwrap();

        // Assert
        assert!(ctx.repo_dir().exists());
        assert!(ctx.config_path().exists());
        assert!(!ctx.repo_dir().join(".git").exists());
    }

    #[test]
    fn test_init_strip_git_false_preserves_git_directory() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        assert!(ctx.repo_dir().join(".git").exists());
    }

    #[test]
    fn test_init_strip_git_without_url_is_noop() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, true).unwrap();

        // Assert — scaffold mode ignores strip_git
        assert!(ctx.repo_dir().exists());
        assert!(ctx.config_path().exists());
    }
}
