use crate::config::Config;
use crate::context::Context;
use std::fs;

pub fn run(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let packages_dir = ctx.packages_dir();
    let config_path = ctx.config_path();

    if config_path.exists() {
        println!("Already initialized at {}", ctx.default_repo_dir().display());
        return Ok(());
    }

    fs::create_dir_all(&packages_dir)?;

    let config = Config::default();
    config.save(&config_path)?;

    println!("Initialized homeos at {}", ctx.default_repo_dir().display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
        (tmp, ctx)
    }

    #[test]
    fn test_init_creates_structure() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx).unwrap();

        // Assert
        assert!(ctx.default_repo_dir().exists());
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
}
