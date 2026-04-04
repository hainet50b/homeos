use crate::config::Config;
use crate::context::Context;
use std::io::Write;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_to(ctx, &mut std::io::stdout())
}

fn list_to<W: Write>(ctx: &Context, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    if config.plugins.is_empty() {
        return Ok(());
    }

    let name_width = config
        .plugins
        .keys()
        .map(|n| n.len())
        .max()
        .unwrap_or(0)
        .max(4); // "Name" header length

    writeln!(writer, "{:<name_width$}  URL", "Name")?;
    writeln!(writer, "{:<name_width$}  ---", "-".repeat(name_width))?;

    for (name, plugin) in &config.plugins {
        writeln!(writer, "{:<name_width$}  {}", name, plugin.url)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    fn fixture(base_dir: &TempDir) -> Context {
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());
        std::fs::create_dir_all(ctx.repo_dir()).unwrap();
        ctx
    }

    #[test]
    fn test_list_no_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(String::from_utf8(output).unwrap(), "");
    }

    #[test]
    fn test_list_single_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-mise".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("https://github.com/hainet50b/homeos-plugin-mise"));
    }

    #[test]
    fn test_list_multiple_plugins_sorted() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "rustup".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-rustup".to_string(),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-mise".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Header + separator + 2 plugins
        assert_eq!(lines.len(), 4);
        // BTreeMap sorts alphabetically: mise before rustup
        assert!(lines[2].starts_with("mise"));
        assert!(lines[3].starts_with("rustup"));
    }

    #[test]
    fn test_list_table_header_format() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: "https://example.com".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("Name"));
        assert!(lines[0].contains("URL"));
        assert!(lines[1].starts_with("----"));
        assert!(lines[1].contains("---"));
    }

    #[test]
    fn test_list_name_column_width_adjusts() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "long-plugin-name".to_string(),
            PluginConfig {
                url: "https://example.com".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Separator dashes should be at least as long as "long-plugin-name" (16 chars)
        assert!(lines[1].starts_with(&"-".repeat(16)));
    }

    #[test]
    fn test_list_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_err());
    }
}
