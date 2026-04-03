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

    fn test_context(base: &TempDir) -> Context {
        Context::new(Some(base.path().to_path_buf()))
    }

    #[test]
    fn test_init_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        run(&ctx).unwrap();

        assert!(ctx.default_repo_dir().exists());
        assert!(ctx.packages_dir().exists());
        assert!(ctx.config_path().exists());

        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_init_idempotent() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        run(&ctx).unwrap();

        // Modify config to verify init doesn't overwrite
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.packages.insert(
            "test".to_string(),
            crate::config::PackageConfig::default(),
        );
        config.save(&ctx.config_path()).unwrap();

        run(&ctx).unwrap();

        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages.len(), 1);
    }

    #[test]
    fn test_init_directory_paths() {
        let tmp = TempDir::new().unwrap();
        let ctx = test_context(&tmp);

        run(&ctx).unwrap();

        let base = tmp.path();
        assert!(base.join("repos/default/packages").exists());
        assert!(base.join("repos/default/homeos.yml").exists());
    }
}
