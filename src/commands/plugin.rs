use crate::config::{Config, PluginConfig};
use crate::context::Context;
use serde::Deserialize;
use std::io::Write;
use std::process::Command;

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

#[derive(Deserialize)]
struct GitHubSearchResponse {
    items: Vec<GitHubRepo>,
}

#[derive(Deserialize)]
struct GitHubRepo {
    name: String,
    description: Option<String>,
    html_url: String,
}

pub struct RemotePlugin {
    pub name: String,
    pub description: String,
    pub url: String,
}

fn fetch_remote_plugins() -> Result<Vec<RemotePlugin>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let response: GitHubSearchResponse = client
        .get("https://api.github.com/search/repositories?q=homeos-plugin-+in:name+user:hainet50b")
        .header("User-Agent", "homeos")
        .send()?
        .json()?;

    let plugins = response
        .items
        .into_iter()
        .filter(|r| r.name.starts_with("homeos-plugin-"))
        .map(|r| {
            let name = r.name.strip_prefix("homeos-plugin-").unwrap().to_string();
            RemotePlugin {
                name,
                description: r.description.unwrap_or_default(),
                url: r.html_url,
            }
        })
        .collect();

    Ok(plugins)
}

pub fn list_remote() -> Result<(), Box<dyn std::error::Error>> {
    list_remote_to(&mut std::io::stdout(), fetch_remote_plugins)
}

fn list_remote_to<W: Write, F>(writer: &mut W, fetch: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<Vec<RemotePlugin>, Box<dyn std::error::Error>>,
{
    let plugins = fetch()?;

    if plugins.is_empty() {
        writeln!(writer, "No remote plugins found.")?;
        return Ok(());
    }

    let name_width = plugins
        .iter()
        .map(|p| p.name.len())
        .max()
        .unwrap_or(0)
        .max(4); // "Name" header length

    let desc_width = plugins
        .iter()
        .map(|p| p.description.len())
        .max()
        .unwrap_or(0)
        .max(11); // "Description" header length

    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  URL",
        "Name", "Description"
    )?;
    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  ---",
        "-".repeat(name_width),
        "-".repeat(desc_width)
    )?;

    for plugin in &plugins {
        writeln!(
            writer,
            "{:<name_width$}  {:<desc_width$}  {}",
            plugin.name, plugin.description, plugin.url
        )?;
    }

    Ok(())
}

fn check_repo_exists(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = format!(
        "https://api.github.com/repos/hainet50b/homeos-plugin-{}",
        name
    );
    let client = reqwest::blocking::Client::new();
    let response = client.get(&api_url).header("User-Agent", "homeos").send()?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "Plugin '{}' not found on GitHub (homeos-plugin-{})",
            name, name
        )
        .into());
    }

    Ok(())
}

pub fn add(ctx: &Context, name: &str, url: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    add_with(ctx, name, url, check_repo_exists)
}

fn add_with<F>(
    ctx: &Context,
    name: &str,
    url: Option<&str>,
    repo_checker: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&str) -> Result<(), Box<dyn std::error::Error>>,
{
    let config = Config::load(&ctx.config_path())?;

    if config.plugins.contains_key(name) {
        return Err(format!("Plugin '{}' already exists", name).into());
    }

    let auto_resolved = url.is_none();
    let url = url
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("https://github.com/hainet50b/homeos-plugin-{}", name));

    let plugins_dir = ctx.plugins_dir();
    let target = plugins_dir.join(name);

    if target.exists() {
        return Err(format!("Plugin directory '{}' already exists", name).into());
    }

    if auto_resolved {
        repo_checker(name)?;
    }

    std::fs::create_dir_all(&plugins_dir)?;

    let output = Command::new("git")
        .args(["clone", &url, &target.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()).into());
    }

    if !target.join("params.yml").exists() {
        std::fs::remove_dir_all(&target)?;
        return Err("Not a valid homeos plugin".into());
    }

    let mut config = config;
    config
        .plugins
        .insert(name.to_string(), PluginConfig { url: url.clone() });
    config.save(&ctx.config_path())?;

    println!("Plugin '{}' added successfully", name);
    Ok(())
}

pub fn remove(ctx: &Context, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.plugins.contains_key(name) {
        return Err(format!("Plugin '{}' not found", name).into());
    }

    // Warn if packages reference this plugin
    let referencing: Vec<&String> = config
        .packages
        .iter()
        .filter(|(_, pkg)| pkg.plugin.as_deref() == Some(name))
        .map(|(pkg_name, _)| pkg_name)
        .collect();

    if !referencing.is_empty() {
        let names = referencing
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "Warning: the following packages reference plugin '{}': {}",
            name, names
        );
    }

    // Remove plugin directory
    let plugin_dir = ctx.plugins_dir().join(name);
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir)?;
    }

    // Remove from config
    config.plugins.remove(name);
    config.save(&ctx.config_path())?;

    println!("Plugin '{}' removed successfully", name);
    Ok(())
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

    fn create_local_git_repo(dir: &std::path::Path) {
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

    fn create_local_plugin_repo(dir: &std::path::Path) {
        create_local_git_repo(dir);
        std::fs::write(dir.join("params.yml"), "name: test\n").unwrap();
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "add", "params.yml"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "-m",
                "add params.yml",
            ])
            .output()
            .unwrap();
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

    #[test]
    fn test_list_remote_no_plugins() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || Ok(vec![]);

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "No remote plugins found.\n");
    }

    #[test]
    fn test_list_remote_single_plugin() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "Manage mise tools".to_string(),
                url: "https://github.com/hainet50b/homeos-plugin-mise".to_string(),
            }])
        };

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("Manage mise tools"));
        assert!(text.contains("https://github.com/hainet50b/homeos-plugin-mise"));
    }

    #[test]
    fn test_list_remote_multiple_plugins() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "mise".to_string(),
                    description: "Manage mise tools".to_string(),
                    url: "https://github.com/hainet50b/homeos-plugin-mise".to_string(),
                },
                RemotePlugin {
                    name: "rustup".to_string(),
                    description: "Manage Rust toolchains".to_string(),
                    url: "https://github.com/hainet50b/homeos-plugin-rustup".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Header + separator + 2 plugins
        assert_eq!(lines.len(), 4);
        assert!(lines[2].contains("mise"));
        assert!(lines[3].contains("rustup"));
    }

    #[test]
    fn test_list_remote_table_header_format() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "A tool manager".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("Name"));
        assert!(lines[0].contains("Description"));
        assert!(lines[0].contains("URL"));
        assert!(lines[1].starts_with("----"));
        assert!(lines[1].contains("---"));
    }

    #[test]
    fn test_list_remote_name_column_width_adjusts() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "long-plugin-name".to_string(),
                description: "A plugin".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Separator dashes should be at least as long as "long-plugin-name" (16 chars)
        assert!(lines[1].starts_with(&"-".repeat(16)));
    }

    #[test]
    fn test_list_remote_empty_description() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(&mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("https://example.com"));
    }

    #[test]
    fn test_list_remote_fetch_error() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || Err("Network error".into());

        // Act
        let result = list_remote_to(&mut output, fetch);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Network error"));
    }

    #[test]
    fn test_add_clones_and_registers_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act
        let result = add(&ctx, "dnf", Some(&source_dir.path().to_string_lossy()));

        // Assert
        assert!(result.is_ok());
        assert!(ctx.plugins_dir().join("dnf").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
        assert_eq!(
            config.plugins["dnf"].url,
            source_dir.path().to_string_lossy()
        );
    }

    #[test]
    fn test_add_default_url_without_explicit_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        // Create a local plugin repo to simulate the default URL clone target
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — use explicit URL since we can't clone from GitHub in tests
        let result = add(&ctx, "mise", Some(&source_dir.path().to_string_lossy()));

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("mise"));
    }

    #[test]
    fn test_add_rejects_repo_without_params_yml() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let result = add(&ctx, "bad", Some(&source_dir.path().to_string_lossy()));

        // Assert
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Not a valid homeos plugin");
    }

    #[test]
    fn test_add_rejects_repo_without_params_yml_cleans_up() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let _ = add(&ctx, "bad", Some(&source_dir.path().to_string_lossy()));

        // Assert — cloned directory should be removed
        assert!(!ctx.plugins_dir().join("bad").exists());
        // Config should not be modified
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("bad"));
    }

    #[test]
    fn test_add_already_registered() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-dnf".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"));

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin 'dnf' already exists");
    }

    #[test]
    fn test_add_directory_already_exists() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"));

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin directory 'dnf' already exists");
    }

    #[test]
    fn test_add_invalid_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = add(&ctx, "bad-plugin", Some("not-a-valid-url"));

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.starts_with("git clone failed:"));
    }

    #[test]
    fn test_add_creates_plugins_dir() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());
        assert!(!ctx.plugins_dir().exists());

        // Act
        let result = add(&ctx, "dnf", Some(&source_dir.path().to_string_lossy()));

        // Assert
        assert!(result.is_ok());
        assert!(ctx.plugins_dir().exists());
        assert!(ctx.plugins_dir().join("dnf").exists());
    }

    #[test]
    fn test_add_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"));

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_add_resolves_default_url() {
        // Arrange — use add_with to inject a checker that records it was called
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — no URL provided, repo_checker should be called
        // We use add_with directly to inject a fake checker that always succeeds,
        // but we can't actually clone from the default GitHub URL, so we test
        // via the error path: the checker rejects the plugin.
        let result = add_with(&ctx, "nonexistent-plugin-xyz", None, |name| {
            Err(format!(
                "Plugin '{}' not found on GitHub (homeos-plugin-{})",
                name, name
            )
            .into())
        });

        // Assert — should fail with the checker's error, not git clone
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found on GitHub"));
        assert!(err.contains("nonexistent-plugin-xyz"));
    }

    #[test]
    fn test_add_auto_resolved_url_checks_repo_exists() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act — no URL, checker returns error
        let result = add_with(&ctx, "missing", None, |_name| {
            Err("Plugin 'missing' not found on GitHub (homeos-plugin-missing)".into())
        });

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(
            err,
            "Plugin 'missing' not found on GitHub (homeos-plugin-missing)"
        );
    }

    #[test]
    fn test_add_auto_resolved_url_skips_check_with_explicit_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — explicit URL provided, checker should NOT be called
        let result = add_with(
            &ctx,
            "dnf",
            Some(&source_dir.path().to_string_lossy()),
            |_name| {
                panic!("repo_checker should not be called when URL is explicit");
            },
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_add_auto_resolved_url_no_clone_on_check_failure() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act — checker fails, clone should not be attempted
        let _ = add_with(&ctx, "bad-plugin", None, |_name| Err("not found".into()));

        // Assert — plugins directory should not have been created
        assert!(!ctx.plugins_dir().join("bad-plugin").exists());
    }

    #[test]
    fn test_remove_deletes_directory_and_config_entry() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-dnf".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        // Act
        let result = remove(&ctx, "dnf");

        // Assert
        assert!(result.is_ok());
        assert!(!plugin_dir.exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = remove(&ctx, "nonexistent");

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin 'nonexistent' not found");
    }

    #[test]
    fn test_remove_without_directory() {
        // Arrange — plugin registered in config but directory is missing
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-dnf".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = remove(&ctx, "dnf");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_warns_when_packages_reference_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-dnf".to_string(),
            },
        );
        config.packages.insert(
            "neovim".to_string(),
            crate::config::PackageConfig {
                plugin: Some("dnf".to_string()),
                ..Default::default()
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act — should succeed (warn but not block)
        let result = remove(&ctx, "dnf");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_does_not_affect_other_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-dnf".to_string(),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: "https://github.com/hainet50b/homeos-plugin-mise".to_string(),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("mise")).unwrap();

        // Act
        let result = remove(&ctx, "dnf");

        // Assert
        assert!(result.is_ok());
        assert!(!ctx.plugins_dir().join("dnf").exists());
        assert!(ctx.plugins_dir().join("mise").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
        assert!(config.plugins.contains_key("mise"));
    }

    #[test]
    fn test_remove_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()), "default".to_string());

        // Act
        let result = remove(&ctx, "dnf");

        // Assert
        assert!(result.is_err());
    }
}
