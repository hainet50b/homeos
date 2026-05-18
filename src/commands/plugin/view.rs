use crate::config::{Config, PluginManifest};
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use std::io::Write;
use std::process::Command;

pub fn info(ctx: &Context, plugin: &str) -> Result<(), Box<dyn std::error::Error>> {
    info_to(ctx, plugin, &mut std::io::stdout())
}

fn info_to<W: Write>(
    ctx: &Context,
    plugin: &str,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let plugin_config = config.plugins.get(plugin).ok_or_else(|| {
        HomeosError::new(
            reasons::PLUGIN_NOT_FOUND,
            format!("Plugin '{plugin}' not found"),
        )
    })?;

    let plugin_dir = ctx.plugins_dir().join(plugin);

    let manifest_path = plugin_dir.join("plugin.yml");
    let manifest = if manifest_path.is_file() {
        PluginManifest::load(&manifest_path).ok()
    } else {
        None
    };
    let description = manifest
        .as_ref()
        .map(|m| m.description.as_str())
        .unwrap_or("");
    let params: Vec<String> = manifest
        .as_ref()
        .map(|m| m.params.clone())
        .unwrap_or_default();

    writeln!(writer, "Plugin: {plugin}")?;
    writeln!(writer, "Description: {description}")?;
    writeln!(
        writer,
        "URL: {}",
        plugin_config.url.as_deref().unwrap_or("(local)")
    )?;

    writeln!(writer, "Parameters:")?;
    if params.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for p in &params {
            writeln!(writer, "  {p}")?;
        }
    }

    let actions = ["install", "update", "uninstall"];
    let extensions = ["sh", "ps1"];

    writeln!(writer, "Templates:")?;
    for action in &actions {
        for ext in &extensions {
            let filename = format!("{action}.{ext}.tmpl");
            let file_path = plugin_dir.join(&filename);
            if file_path.is_file() {
                writeln!(writer, "  {filename} ({})", file_path.display())?;
            } else {
                writeln!(writer, "  {filename} (not found)")?;
            }
        }
    }

    Ok(())
}

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
        return Err(HomeosError::new(
            reasons::PLUGIN_NOT_FOUND,
            format!("Plugin '{plugin}' not found"),
        )
        .into());
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
                return Err(HomeosError::new(
                    reasons::PLUGIN_NOT_FOUND,
                    format!("Plugin '{plugin_name}' not found"),
                )
                .into());
            }
            ctx.plugins_dir().join(plugin_name)
        }
        None => ctx.plugins_dir(),
    };

    if !dir.exists() {
        return Err(HomeosError::new(
            reasons::DIRECTORY_NOT_FOUND,
            format!("Directory not found at {}", dir.display()),
        )
        .into());
    }

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use crate::error::reasons;
    use tempfile::TempDir;

    fn fixture(base_dir: &TempDir) -> Context {
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
        std::fs::create_dir_all(ctx.data_dir()).unwrap();
        ctx
    }

    fn fixture_with_config(base_dir: &TempDir) -> Context {
        let ctx = fixture(base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        ctx
    }

    #[test]
    fn test_info_displays_plugin_details() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/hainet50b/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: DNF package manager plugin for homeos.\nparams:\n  - name\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Plugin: dnf"));
        assert!(text.contains("Description: DNF package manager plugin for homeos."));
        assert!(text.contains("URL: https://github.com/hainet50b/homeos-plugin-dnf"));
        assert!(text.contains("Parameters:"));
        assert!(text.contains("  name"));
        assert!(text.contains("Templates:"));
    }

    #[test]
    fn test_info_shows_local_when_url_is_none() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config
            .plugins
            .insert("custom".to_string(), PluginConfig { url: None });
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "custom", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("URL: (local)"));
    }

    #[test]
    fn test_info_shows_none_when_params_empty() {
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
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: A plugin\nparams: []\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Parameters:\n  (none)"));
    }

    #[test]
    fn test_info_shows_none_when_plugin_yml_missing() {
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
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Parameters:\n  (none)"));
    }

    #[test]
    fn test_info_lists_templates_with_full_path_when_present() {
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
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: A plugin\nparams: []\n",
        )
        .unwrap();
        std::fs::write(plugin_dir.join("install.sh.tmpl"), "sh install\n").unwrap();
        std::fs::write(plugin_dir.join("update.ps1.tmpl"), "ps1 update\n").unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let install_sh_tmpl = plugin_dir.join("install.sh.tmpl");
        let update_ps1_tmpl = plugin_dir.join("update.ps1.tmpl");
        assert!(text.contains(&format!(
            "  install.sh.tmpl ({})",
            install_sh_tmpl.display()
        )));
        assert!(text.contains("  install.ps1.tmpl (not found)"));
        assert!(text.contains("  update.sh.tmpl (not found)"));
        assert!(text.contains(&format!(
            "  update.ps1.tmpl ({})",
            update_ps1_tmpl.display()
        )));
        assert!(text.contains("  uninstall.sh.tmpl (not found)"));
        assert!(text.contains("  uninstall.ps1.tmpl (not found)"));
    }

    #[test]
    fn test_info_lists_all_templates_not_found_when_none_exist() {
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
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Templates:"));
        assert!(text.contains("  install.sh.tmpl (not found)"));
        assert!(text.contains("  install.ps1.tmpl (not found)"));
        assert!(text.contains("  update.sh.tmpl (not found)"));
        assert!(text.contains("  update.ps1.tmpl (not found)"));
        assert!(text.contains("  uninstall.sh.tmpl (not found)"));
        assert!(text.contains("  uninstall.ps1.tmpl (not found)"));
    }

    #[test]
    fn test_info_displays_description_from_plugin_yml() {
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
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: DNF package manager plugin for homeos.\nparams: []\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        info_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Description: DNF package manager plugin for homeos."));
    }

    #[test]
    fn test_info_shows_empty_description_when_plugin_yml_missing() {
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
        let mut output = Vec::new();

        // Act
        info_to(&ctx, "dnf", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Description: \n"));
    }

    #[test]
    fn test_info_errors_when_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "nonexistent", &mut output);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Plugin 'nonexistent' not found"
        );
    }

    #[test]
    fn test_info_plugin_not_found_reason() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "nonexistent", &mut output);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::PLUGIN_NOT_FOUND);
    }

    #[test]
    fn test_info_errors_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "dnf", &mut output);

        // Assert
        assert!(result.is_err());
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
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
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
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));

        // Act
        let result = resolve_cd_target(&ctx, None);

        // Assert
        assert!(result.is_err());
    }
}
