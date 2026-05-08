use crate::config::Config;
use crate::context::Context;
use std::io::Write;
use std::process::Command;

pub fn cat(ctx: &Context, plugin: &str) -> Result<(), Box<dyn std::error::Error>> {
    cat_to(ctx, plugin, &mut std::io::stdout())
}

fn cat_to<W: Write>(
    ctx: &Context,
    plugin: &str,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    if !config.plugins.contains_key(plugin) {
        return Err(format!("Plugin '{plugin}' not found").into());
    }

    let plugin_dir = ctx.plugins_dir().join(plugin);

    // Display plugin.yml
    let plugin_yml_path = plugin_dir.join("plugin.yml");
    writeln!(writer, "=== plugin.yml ===")?;
    if plugin_yml_path.is_file() {
        let content = std::fs::read_to_string(&plugin_yml_path)?;
        write!(writer, "{content}")?;
    } else {
        writeln!(writer, "(not found)")?;
    }

    // Display template files for each action and extension
    let actions = ["install", "update", "uninstall"];
    let extensions = ["sh", "ps1"];

    for action in &actions {
        for ext in &extensions {
            let filename = format!("{action}.{ext}.tmpl");
            let file_path = plugin_dir.join(&filename);
            if file_path.is_file() {
                writeln!(writer)?;
                writeln!(writer, "=== {filename} ===")?;
                let content = std::fs::read_to_string(&file_path)?;
                write!(writer, "{content}")?;
            }
        }
    }

    Ok(())
}

pub fn cd(ctx: &Context, plugin: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_cd_target(ctx, plugin)?;
    let shell = crate::commands::detect_shell();

    let status = Command::new(&shell).current_dir(&dir).status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn resolve_cd_target(
    ctx: &Context,
    plugin: Option<&str>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let dir = match plugin {
        Some(plugin_name) => {
            if !config.plugins.contains_key(plugin_name) {
                return Err(format!("Plugin '{plugin_name}' not found").into());
            }
            ctx.plugins_dir().join(plugin_name)
        }
        None => ctx.plugins_dir(),
    };

    if !dir.exists() {
        return Err(format!("Directory not found at {}", dir.display()).into());
    }

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use tempfile::TempDir;

    fn fixture(base_dir: &TempDir) -> Context {
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());
        std::fs::create_dir_all(ctx.repo_dir()).unwrap();
        ctx
    }

    fn fixture_with_config(base_dir: &TempDir) -> Context {
        let ctx = fixture(base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        ctx
    }

    #[test]
    fn test_cat_displays_plugin_yml_and_templates() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.yml"), "params:\n  - name\n").unwrap();
        std::fs::write(
            plugin_dir.join("install.sh.tmpl"),
            "#!/usr/bin/env sh\nsudo dnf install -y {{name}}\n",
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join("uninstall.sh.tmpl"),
            "#!/usr/bin/env sh\nsudo dnf remove -y {{name}}\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        cat_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("=== plugin.yml ==="));
        assert!(text.contains("params:\n  - name"));
        assert!(text.contains("=== install.sh.tmpl ==="));
        assert!(text.contains("sudo dnf install -y {{name}}"));
        assert!(text.contains("=== uninstall.sh.tmpl ==="));
        assert!(text.contains("sudo dnf remove -y {{name}}"));
    }

    #[test]
    fn test_cat_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "nonexistent", &mut output);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Plugin 'nonexistent' not found"
        );
    }

    #[test]
    fn test_cat_missing_plugin_yml_shows_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let mut output = Vec::new();

        // Act
        cat_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("=== plugin.yml ==="));
        assert!(text.contains("(not found)"));
    }

    #[test]
    fn test_cat_only_shows_existing_templates() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.yml"), "params: []\n").unwrap();
        std::fs::write(plugin_dir.join("install.sh.tmpl"), "echo install\n").unwrap();
        let mut output = Vec::new();

        // Act
        cat_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("=== install.sh.tmpl ==="));
        assert!(!text.contains("update.sh.tmpl"));
        assert!(!text.contains("uninstall.sh.tmpl"));
    }

    #[test]
    fn test_cat_shows_both_sh_and_ps1_templates() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.yml"), "params: []\n").unwrap();
        std::fs::write(plugin_dir.join("install.sh.tmpl"), "sh install\n").unwrap();
        std::fs::write(plugin_dir.join("install.ps1.tmpl"), "ps1 install\n").unwrap();
        let mut output = Vec::new();

        // Act
        cat_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("=== install.sh.tmpl ==="));
        assert!(text.contains("sh install"));
        assert!(text.contains("=== install.ps1.tmpl ==="));
        assert!(text.contains("ps1 install"));
    }

    #[test]
    fn test_cat_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_cd_target_returns_plugins_dir_when_no_name() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        std::fs::create_dir_all(ctx.plugins_dir()).unwrap();

        // Act
        let result = resolve_cd_target(&ctx, None).unwrap();

        // Assert
        assert_eq!(result, ctx.plugins_dir());
    }

    #[test]
    fn test_resolve_cd_target_returns_plugin_dir_when_name_given() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();

        // Act
        let result = resolve_cd_target(&ctx, Some("dnf")).unwrap();

        // Assert
        assert_eq!(result, ctx.plugins_dir().join("dnf"));
    }

    #[test]
    fn test_resolve_cd_target_errors_when_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = resolve_cd_target(&ctx, Some("nonexistent"));

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Plugin 'nonexistent' not found"));
    }

    #[test]
    fn test_resolve_cd_target_errors_when_plugins_dir_missing() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = resolve_cd_target(&ctx, None);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Directory not found"));
    }

    #[test]
    fn test_resolve_cd_target_errors_when_plugin_dir_missing() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = resolve_cd_target(&ctx, Some("dnf"));

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Directory not found"));
    }

    #[test]
    fn test_resolve_cd_target_errors_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());

        // Act
        let result = resolve_cd_target(&ctx, None);

        // Assert
        assert!(result.is_err());
    }
}
