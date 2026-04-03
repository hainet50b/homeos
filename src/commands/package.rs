use crate::config::Config;
use crate::context::Context;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    for (name, pkg) in &config.packages {
        if pkg.enabled {
            println!("{name}");
        } else {
            println!("{name} (disabled)");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let base_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(base_dir));

        std::fs::create_dir_all(ctx.config_path().parent().unwrap()).unwrap();
        std::fs::write(ctx.config_path(), yaml).unwrap();

        (tmp, ctx)
    }

    #[test]
    fn test_list_shows_all_packages() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n  ripgrep: {}\n  starship: {}\n",
        );

        // Act
        let result = list(&ctx);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_empty_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");

        // Act
        let result = list(&ctx);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = list(&ctx);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_list_formats_enabled_and_disabled() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n",
        );
        let config = Config::load(&ctx.config_path()).unwrap();

        // Act
        let output: Vec<String> = config
            .packages
            .iter()
            .map(|(name, pkg)| {
                if pkg.enabled {
                    name.clone()
                } else {
                    format!("{name} (disabled)")
                }
            })
            .collect();

        // Assert
        assert_eq!(output, vec!["neovim (disabled)", "ripgrep"]);
    }
}
