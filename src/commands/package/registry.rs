use crate::config::{Config, PackageConfig};
use crate::context::Context;
use crate::state::State;
use std::io::Write;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_to(ctx, &mut std::io::stdout())
}

fn list_to<W: Write>(ctx: &Context, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let state_path = ctx.state_path();
    let installed_packages: Vec<String> = if state_path.exists() {
        State::load(&state_path)?.installed
    } else {
        Vec::new()
    };

    if config.packages.is_empty() {
        return Ok(());
    }

    let name_width = config
        .packages
        .keys()
        .map(|n| n.len())
        .max()
        .unwrap_or(0)
        .max(7); // "Package" header length

    writeln!(
        writer,
        "{:<name_width$}  {:<7}  Installed",
        "Package", "Enabled"
    )?;
    writeln!(
        writer,
        "{:<name_width$}  {:<7}  ---------",
        "-".repeat(name_width),
        "-------"
    )?;

    for (name, pkg) in &config.packages {
        let enabled = if pkg.enabled { "yes" } else { "no" };
        let installed = if installed_packages.contains(&name.to_string()) {
            "yes"
        } else {
            "no"
        };
        writeln!(
            writer,
            "{:<name_width$}  {:<7}  {}",
            name, enabled, installed
        )?;
    }

    Ok(())
}

pub fn remove(ctx: &Context, package: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let state_path = ctx.state_path();
    if state_path.exists() {
        let state = State::load(&state_path)?;
        if state.installed.contains(&package.to_string()) {
            return Err(format!(
                "Package '{package}' is currently installed. Uninstall it first with: homeos package uninstall {package}"
            )
            .into());
        }
    }

    config.packages.remove(package);
    config.save(&ctx.config_path())?;

    println!("Removed package '{package}'");
    Ok(())
}

pub fn add(ctx: &Context, package: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if config.packages.contains_key(package) {
        return Err(format!("Package '{package}' already exists").into());
    }

    config
        .packages
        .insert(package.to_string(), PackageConfig::default());
    config.save(&ctx.config_path())?;

    let pkg_dir = ctx.packages_dir().join(package);
    std::fs::create_dir_all(&pkg_dir)?;

    for (action, ext) in skeleton_scripts() {
        let filename = format!("{action}.{ext}");
        let path = pkg_dir.join(&filename);
        if !path.exists() {
            let content = skeleton_script_content(action, ext, package);
            std::fs::write(path, content)?;
        }
    }

    println!("Added package '{package}'");
    Ok(())
}

pub fn enable(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    for package in packages {
        let pkg = config
            .packages
            .get_mut(package.as_str())
            .ok_or_else(|| format!("Package '{package}' not found"))?;

        if pkg.enabled {
            println!("Package '{package}' is already enabled");
            continue;
        }

        pkg.enabled = true;
        println!("Enabled package '{package}'");
    }

    config.save(&ctx.config_path())?;
    Ok(())
}

pub fn disable(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    for package in packages {
        let pkg = config
            .packages
            .get_mut(package.as_str())
            .ok_or_else(|| format!("Package '{package}' not found"))?;

        if !pkg.enabled {
            println!("Package '{package}' is already disabled");
            continue;
        }

        pkg.enabled = false;
        println!("Disabled package '{package}'");
    }

    config.save(&ctx.config_path())?;
    Ok(())
}

pub fn cat(ctx: &Context, package: &str) -> Result<(), Box<dyn std::error::Error>> {
    cat_to(ctx, package, &mut std::io::stdout())
}

fn cat_to<W: Write>(ctx: &Context, package: &str, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let actions = ["install", "update", "uninstall"];
    let ext = if cfg!(windows) { "ps1" } else { "sh" };
    let pkg_dir = ctx.packages_dir().join(package);

    for (i, action) in actions.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let filename = format!("{action}.{ext}");
        writeln!(writer, "=== {filename} ===")?;
        let script_path = pkg_dir.join(&filename);
        if script_path.is_file() {
            let content = std::fs::read_to_string(&script_path)?;
            write!(writer, "{content}")?;
        } else {
            writeln!(writer, "(not found)")?;
        }
    }

    Ok(())
}

fn skeleton_scripts() -> Vec<(&'static str, &'static str)> {
    let actions = ["install", "update", "uninstall"];
    let ext = if cfg!(windows) { "ps1" } else { "sh" };
    actions.iter().map(|a| (*a, ext)).collect()
}

fn skeleton_script_content(action: &str, ext: &str, package: &str) -> String {
    if ext == "ps1" {
        format!("# Generated by homeos — fill in the {action} logic for {package}.\n")
    } else {
        format!(
            "#!/usr/bin/env sh\n\
             # Generated by homeos — fill in the {action} logic for {package}.\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;
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
    fn test_list_shows_table_with_all_packages() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n  ripgrep: {}\n  starship: {}\n",
        );
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Package"));
        assert!(text.contains("Enabled"));
        assert!(text.contains("Installed"));
        assert!(text.contains("neovim"));
        assert!(text.contains("ripgrep"));
        assert!(text.contains("starship"));
    }

    #[test]
    fn test_list_empty_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_list_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_list_shows_enabled_and_disabled_status() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n",
        );
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // neovim is disabled, ripgrep is enabled
        assert!(lines[2].contains("neovim") && lines[2].contains("no"));
        assert!(lines[3].contains("ripgrep") && lines[3].contains("yes"));
    }

    #[test]
    fn test_list_shows_installed_status() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n  ripgrep: {}\n",
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // neovim: enabled=yes, installed=yes
        assert!(lines[2].contains("neovim"));
        assert!(lines[2].ends_with("yes"));
        // ripgrep: enabled=yes, installed=no
        assert!(lines[3].contains("ripgrep"));
        assert!(lines[3].ends_with("no"));
    }

    #[test]
    fn test_list_without_state_file() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n",
        );
        // No state.yml created
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // neovim: enabled=yes, installed=no (no state file means not installed)
        assert!(lines[2].contains("neovim"));
        assert!(lines[2].ends_with("no"));
    }

    #[test]
    fn test_add_creates_package_dir_and_config_entry() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
        assert!(ctx.packages_dir().join("neovim").is_dir());
    }

    #[test]
    fn test_add_generates_skeleton_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(&ctx, "neovim").unwrap();

        // Assert
        let pkg_dir = ctx.packages_dir().join("neovim");
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        for action in &["install", "update", "uninstall"] {
            let script = pkg_dir.join(format!("{action}.{ext}"));
            assert!(script.is_file(), "Expected {action}.{ext} to exist");
        }
    }

    #[test]
    fn test_add_skeleton_scripts_contain_comment() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(&ctx, "neovim").unwrap();

        // Assert
        let pkg_dir = ctx.packages_dir().join("neovim");
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        let content = std::fs::read_to_string(pkg_dir.join(format!("install.{ext}"))).unwrap();
        assert!(content.contains("Generated by homeos"));
        assert!(content.contains("install"));
        assert!(content.contains("neovim"));
    }

    #[test]
    fn test_add_errors_when_package_already_exists() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = add(&ctx, "neovim");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = add(&ctx, "neovim");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_add_preserves_existing_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  ripgrep: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("ripgrep"));
        assert!(config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_add_preserves_existing_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        let custom_content = "#!/usr/bin/env sh\napt install neovim\n";
        std::fs::write(pkg_dir.join(format!("install.{ext}")), custom_content).unwrap();

        // Act
        add(&ctx, "neovim").unwrap();

        // Assert
        let install_content =
            std::fs::read_to_string(pkg_dir.join(format!("install.{ext}"))).unwrap();
        assert_eq!(install_content, custom_content);
        assert!(pkg_dir.join(format!("update.{ext}")).is_file());
        assert!(pkg_dir.join(format!("uninstall.{ext}")).is_file());
    }

    #[test]
    fn test_add_generates_only_missing_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        let custom_update = "#!/usr/bin/env sh\napt upgrade neovim\n";
        let custom_uninstall = "#!/usr/bin/env sh\napt remove neovim\n";
        std::fs::write(pkg_dir.join(format!("update.{ext}")), custom_update).unwrap();
        std::fs::write(pkg_dir.join(format!("uninstall.{ext}")), custom_uninstall).unwrap();

        // Act
        add(&ctx, "neovim").unwrap();

        // Assert
        let install_content =
            std::fs::read_to_string(pkg_dir.join(format!("install.{ext}"))).unwrap();
        assert!(install_content.contains("Generated by homeos"));
        let update_content =
            std::fs::read_to_string(pkg_dir.join(format!("update.{ext}"))).unwrap();
        assert_eq!(update_content, custom_update);
        let uninstall_content =
            std::fs::read_to_string(pkg_dir.join(format!("uninstall.{ext}"))).unwrap();
        assert_eq!(uninstall_content, custom_uninstall);
    }

    #[test]
    fn test_add_preserves_all_existing_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        for action in &["install", "update", "uninstall"] {
            std::fs::write(
                pkg_dir.join(format!("{action}.{ext}")),
                format!("# custom {action}\n"),
            )
            .unwrap();
        }

        // Act
        add(&ctx, "neovim").unwrap();

        // Assert
        for action in &["install", "update", "uninstall"] {
            let content =
                std::fs::read_to_string(pkg_dir.join(format!("{action}.{ext}"))).unwrap();
            assert_eq!(content, format!("# custom {action}\n"));
        }
    }

    #[test]
    fn test_list_table_header_and_separator() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n",
        );
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("Package"));
        assert!(lines[0].contains("Enabled"));
        assert!(lines[0].contains("Installed"));
        // Second line is separator
        assert!(lines[1].contains("-------"));
    }

    #[test]
    fn test_remove_deletes_config_entry() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = remove(&ctx, "nonexistent");

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_rejects_installed_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("currently installed"));
        assert!(err.contains("Uninstall it first"));
        // Config should be unchanged
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_remove_allows_uninstalled_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_remove_works_without_state_file() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        // No state.yml exists

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_enable_sets_enabled_true() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n");

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
    }

    #[test]
    fn test_enable_already_enabled_is_noop() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
    }

    #[test]
    fn test_enable_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = enable(&ctx, &["nonexistent".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_enable_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_enable_preserves_other_fields() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    actions_overrides:\n      update: install\n    enabled: false\n",
        );

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
        assert_eq!(config.packages["neovim"].actions_overrides["update"], "install");
    }

    #[test]
    fn test_enable_multiple_packages() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n  ripgrep:\n    enabled: false\n",
        );

        // Act
        let result = enable(&ctx, &["neovim".to_string(), "ripgrep".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
        assert!(config.packages["ripgrep"].enabled);
    }

    #[test]
    fn test_enable_multiple_with_already_enabled() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim: {}\n  ripgrep:\n    enabled: false\n",
        );

        // Act
        let result = enable(&ctx, &["neovim".to_string(), "ripgrep".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
        assert!(config.packages["ripgrep"].enabled);
    }

    #[test]
    fn test_enable_multiple_errors_on_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n",
        );

        // Act
        let result = enable(&ctx, &["neovim".to_string(), "nonexistent".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_disable_sets_enabled_false() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
    }

    #[test]
    fn test_disable_already_disabled_is_noop() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n");

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
    }

    #[test]
    fn test_disable_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = disable(&ctx, &["nonexistent".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_disable_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_preserves_other_fields() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    actions_overrides:\n      update: install\n",
        );

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
        assert_eq!(config.packages["neovim"].actions_overrides["update"], "install");
    }

    #[test]
    fn test_disable_multiple_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");

        // Act
        let result = disable(&ctx, &["neovim".to_string(), "ripgrep".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
        assert!(!config.packages["ripgrep"].enabled);
    }

    #[test]
    fn test_disable_multiple_with_already_disabled() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n",
        );

        // Act
        let result = disable(&ctx, &["neovim".to_string(), "ripgrep".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
        assert!(!config.packages["ripgrep"].enabled);
    }

    #[test]
    fn test_disable_multiple_errors_on_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = disable(&ctx, &["neovim".to_string(), "nonexistent".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_last_package_leaves_empty_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = remove(&ctx, "neovim");

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_cat_displays_all_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        std::fs::write(pkg_dir.join(format!("install.{ext}")), "#!/usr/bin/env sh\necho install\n").unwrap();
        std::fs::write(pkg_dir.join(format!("update.{ext}")), "#!/usr/bin/env sh\necho update\n").unwrap();
        std::fs::write(pkg_dir.join(format!("uninstall.{ext}")), "#!/usr/bin/env sh\necho uninstall\n").unwrap();
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains(&format!("=== install.{ext} ===")));
        assert!(written.contains("echo install"));
        assert!(written.contains(&format!("=== update.{ext} ===")));
        assert!(written.contains("echo update"));
        assert!(written.contains(&format!("=== uninstall.{ext} ===")));
        assert!(written.contains("echo uninstall"));
    }

    #[test]
    fn test_cat_shows_not_found_for_missing_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        std::fs::write(pkg_dir.join(format!("install.{ext}")), "#!/usr/bin/env sh\necho install\n").unwrap();
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains(&format!("=== install.{ext} ===")));
        assert!(written.contains("echo install"));
        assert!(written.contains(&format!("=== update.{ext} ===\n(not found)")));
        assert!(written.contains(&format!("=== uninstall.{ext} ===\n(not found)")));
    }

    #[test]
    fn test_cat_all_scripts_missing() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        let ext = if cfg!(windows) { "ps1" } else { "sh" };
        assert!(written.contains(&format!("=== install.{ext} ===\n(not found)")));
        assert!(written.contains(&format!("=== update.{ext} ===\n(not found)")));
        assert!(written.contains(&format!("=== uninstall.{ext} ===\n(not found)")));
    }

    #[test]
    fn test_cat_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "nonexistent", &mut output);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_cat_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_err());
    }
}
