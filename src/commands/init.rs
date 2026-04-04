use crate::config::Config;
use crate::context::Context;
use std::fs;

pub fn run(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let packages_dir = ctx.packages_dir();
    let config_path = ctx.config_path();

    if config_path.exists() {
        println!("Already initialized at {}", ctx.repo_dir().display());
        return Ok(());
    }

    fs::create_dir_all(&packages_dir)?;

    let config = Config::default();
    config.save(&config_path)?;

    let gitignore_path = ctx.gitignore_path();
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "state.yml\n")?;
    }

    println!("Initialized homeos at {}", ctx.repo_dir().display());
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

    #[test]
    fn test_init_creates_structure() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx).unwrap();

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
        run(&ctx).unwrap();
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.packages.insert(
            "test".to_string(),
            crate::config::PackageConfig::default(),
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        run(&ctx).unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages.len(), 1);
    }

    #[test]
    fn test_init_directory_paths() {
        // Arrange
        let (tmp, ctx) = fixture();

        // Act
        run(&ctx).unwrap();

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
        run(&ctx).unwrap();

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
        run(&ctx).unwrap();
        fs::write(ctx.gitignore_path(), "state.yml\ncustom\n").unwrap();

        // Act
        run(&ctx).unwrap();

        // Assert
        let content = fs::read_to_string(ctx.gitignore_path()).unwrap();
        assert_eq!(content, "state.yml\ncustom\n");
    }
}
