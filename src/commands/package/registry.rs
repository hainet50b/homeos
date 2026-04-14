use crate::config::{Config, PackageConfig, PluginManifest};
use crate::context::Context;
use crate::plan::prompt_confirm;
use crate::state::State;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};

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

pub fn add(
    ctx: &Context,
    package: &str,
    depends_on: &[String],
    script_aliases: &BTreeMap<String, String>,
    plugin: Option<&str>,
    params: &BTreeMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if config.packages.contains_key(package) {
        return Err(format!("Package '{package}' already exists").into());
    }

    let pkg_config = PackageConfig {
        depends_on: depends_on.to_vec(),
        script_aliases: script_aliases.clone(),
        plugin: plugin.map(|s| s.to_string()),
        params: params.clone(),
        ..Default::default()
    };
    config.packages.insert(package.to_string(), pkg_config);
    config.save(&ctx.config_path())?;

    let pkg_dir = ctx.packages_dir().join(package);
    if pkg_dir.exists() {
        return Err(format!(
            "Package directory '{package}' already exists. Remove it first to re-create."
        )
        .into());
    }
    std::fs::create_dir_all(&pkg_dir)?;

    if let Some(plugin_name) = plugin {
        generate_plugin_scripts(ctx, &pkg_dir, plugin_name, params)?;
    } else {
        generate_skeleton_scripts(&pkg_dir, package)?;
    }

    println!("Added package '{package}'");
    Ok(())
}

fn generate_skeleton_scripts(
    pkg_dir: &std::path::Path,
    package: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (action, ext) in skeleton_scripts() {
        let filename = format!("{action}.{ext}");
        let path = pkg_dir.join(&filename);
        if !path.exists() {
            let content = skeleton_script_content(action, ext, package);
            std::fs::write(path, content)?;
        }
    }
    Ok(())
}

fn generate_plugin_scripts(
    ctx: &Context,
    pkg_dir: &std::path::Path,
    plugin_name: &str,
    params: &BTreeMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_dir = ctx.plugins_dir().join(plugin_name);
    if !plugin_dir.exists() {
        return Err(format!(
            "Plugin '{plugin_name}' not found. Add it first with: homeos plugin add {plugin_name}"
        )
        .into());
    }

    let manifest_path = plugin_dir.join("plugin.yml");
    if manifest_path.exists() {
        let manifest = PluginManifest::load(&manifest_path)?;
        let missing: Vec<&String> = manifest
            .params
            .iter()
            .filter(|p| !params.contains_key(p.as_str()))
            .collect();
        if !missing.is_empty() {
            let list = missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("Missing required plugin parameters: {list}").into());
        }
    }

    let ext = super::script_extension();
    let tmpl_ext = format!("{ext}.tmpl");
    let actions = ["install", "update", "uninstall"];

    for action in &actions {
        let tmpl_filename = format!("{action}.{tmpl_ext}");
        let tmpl_path = plugin_dir.join(&tmpl_filename);
        if !tmpl_path.exists() {
            continue;
        }

        let script_filename = format!("{action}.{ext}");
        let script_path = pkg_dir.join(&script_filename);
        if script_path.exists() {
            continue;
        }

        let template = std::fs::read_to_string(&tmpl_path)?;
        let rendered = render_template(&template, params);
        std::fs::write(script_path, rendered)?;
    }

    Ok(())
}

fn render_template(template: &str, params: &BTreeMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

pub fn remove(
    ctx: &Context,
    packages: &[String],
    purge: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();
    remove_to(ctx, packages, purge, &mut reader, &mut writer)
}

fn remove_to<R: BufRead, W: Write>(
    ctx: &Context,
    packages: &[String],
    purge: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    for package in packages {
        if !config.packages.contains_key(package.as_str()) {
            return Err(format!("Package '{package}' not found").into());
        }
    }

    let state_path = ctx.state_path();
    if state_path.exists() {
        let state = State::load(&state_path)?;
        for package in packages {
            if state.installed.contains(package) {
                return Err(format!(
                    "Package '{package}' is currently installed. Uninstall it first with: homeos package uninstall {package}"
                )
                .into());
            }
        }
    }

    for package in packages {
        let dependents: Vec<&String> = config
            .packages
            .iter()
            .filter(|(name, _)| !packages.contains(name))
            .filter(|(_, pkg)| pkg.depends_on.contains(package))
            .map(|(name, _)| name)
            .collect();

        if !dependents.is_empty() {
            let list = dependents
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Cannot remove package '{package}' because it is depended on by: {list}"
            )
            .into());
        }
    }

    writeln!(
        writer,
        "The following packages will be removed from homeos.yml:"
    )?;
    for package in packages {
        writeln!(writer, "  {package}")?;
    }

    if purge {
        let dirs_to_delete: Vec<&String> = packages
            .iter()
            .filter(|p| ctx.packages_dir().join(p).exists())
            .collect();
        if !dirs_to_delete.is_empty() {
            writeln!(writer, "\nThe following directories will be deleted:")?;
            for package in &dirs_to_delete {
                writeln!(
                    writer,
                    "  {}",
                    ctx.packages_dir().join(package.as_str()).display()
                )?;
            }
        }
    }

    if !prompt_confirm(reader, writer) {
        writeln!(writer, "Aborted.")?;
        return Ok(());
    }

    for package in packages {
        config.packages.remove(package.as_str());
        if purge {
            let pkg_dir = ctx.packages_dir().join(package);
            if pkg_dir.exists() {
                std::fs::remove_dir_all(&pkg_dir)?;
                writeln!(writer, "Removed package '{package}' and removed directory")?;
            } else {
                writeln!(writer, "Removed package '{package}'")?;
            }
        } else {
            writeln!(writer, "Removed package '{package}'")?;
        }
    }
    config.save(&ctx.config_path())?;

    Ok(())
}

pub fn add_dep(
    ctx: &Context,
    package: &str,
    dependencies: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for dependency in dependencies {
        if pkg.depends_on.contains(dependency) {
            println!("Package '{package}' already depends on '{dependency}'");
        } else {
            pkg.depends_on.push(dependency.clone());
            println!("Added dependency '{dependency}' to package '{package}'");
        }
    }

    config.save(&ctx.config_path())?;
    Ok(())
}

pub fn remove_dep(
    ctx: &Context,
    package: &str,
    dependencies: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for dependency in dependencies {
        if let Some(pos) = pkg.depends_on.iter().position(|d| d == dependency) {
            pkg.depends_on.remove(pos);
            println!("Removed dependency '{dependency}' from package '{package}'");
        } else {
            println!("Package '{package}' does not depend on '{dependency}'");
        }
    }

    config.save(&ctx.config_path())?;
    Ok(())
}

pub fn add_alias(
    ctx: &Context,
    package: &str,
    aliases: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for (target, source) in aliases {
        if pkg.script_aliases.contains_key(target) {
            println!("Package '{package}' already has alias '{target}'");
        } else {
            pkg.script_aliases.insert(target.clone(), source.clone());
            println!("Added alias '{target}={source}' to package '{package}'");
        }
    }

    config.save(&ctx.config_path())?;
    Ok(())
}

pub fn remove_alias(
    ctx: &Context,
    package: &str,
    targets: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for target in targets {
        if pkg.script_aliases.remove(target).is_some() {
            println!("Removed alias '{target}' from package '{package}'");
        } else {
            println!("Package '{package}' does not have alias '{target}'");
        }
    }

    config.save(&ctx.config_path())?;
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

fn cat_to<W: Write>(
    ctx: &Context,
    package: &str,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(format!("Package '{package}' not found").into());
    }

    let actions = ["install", "update", "uninstall"];
    let extensions = super::all_script_extensions();
    let pkg_dir = ctx.packages_dir().join(package);

    let mut first = true;
    for action in &actions {
        for ext in extensions {
            if !first {
                writeln!(writer)?;
            }
            first = false;
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
    }

    Ok(())
}

pub fn cd(ctx: &Context, package: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_cd_target(ctx, package)?;
    let shell = crate::commands::detect_shell();

    let status = std::process::Command::new(&shell)
        .current_dir(&dir)
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn resolve_cd_target(
    ctx: &Context,
    package: Option<&str>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let dir = match package {
        Some(pkg) => {
            if !config.packages.contains_key(pkg) {
                return Err(format!("Package '{pkg}' not found").into());
            }
            ctx.packages_dir().join(pkg)
        }
        None => ctx.packages_dir(),
    };

    if !dir.exists() {
        return Err(format!("Directory not found at {}", dir.display()).into());
    }

    Ok(dir)
}

fn skeleton_scripts() -> Vec<(&'static str, &'static str)> {
    let actions = ["install", "update", "uninstall"];
    let extensions = super::all_script_extensions();
    actions
        .iter()
        .flat_map(|a| extensions.iter().map(move |ext| (*a, *ext)))
        .collect()
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
    use crate::commands::package::{all_script_extensions, script_extension};
    use crate::state::State;
    use std::collections::BTreeMap;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let base_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(base_dir), "default".to_string());

        std::fs::create_dir_all(ctx.config_path().parent().unwrap()).unwrap();
        std::fs::write(ctx.config_path(), yaml).unwrap();

        (tmp, ctx)
    }

    #[test]
    fn test_list_shows_table_with_all_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n  starship: {}\n");
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
        assert!(text.contains("Package"));
        assert!(text.contains("Enabled"));
        assert!(text.contains("Installed"));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator only
    }

    #[test]
    fn test_list_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_list_shows_enabled_and_disabled_status() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n");
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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
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
    fn test_list_table_header_and_separator() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
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
    fn test_add_creates_package_dir_and_config_entry() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

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
        add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();

        // Assert
        let pkg_dir = ctx.packages_dir().join("neovim");
        for action in &["install", "update", "uninstall"] {
            for ext in &["sh", "ps1"] {
                let script = pkg_dir.join(format!("{action}.{ext}"));
                assert!(script.is_file(), "Expected {action}.{ext} to exist");
            }
        }
    }

    #[test]
    fn test_add_skeleton_scripts_contain_comment() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();

        // Assert
        let pkg_dir = ctx.packages_dir().join("neovim");
        for ext in &["sh", "ps1"] {
            let content = std::fs::read_to_string(pkg_dir.join(format!("install.{ext}"))).unwrap();
            assert!(content.contains("Generated by homeos"));
            assert!(content.contains("install"));
            assert!(content.contains("neovim"));
        }
    }

    #[test]
    fn test_add_generates_skeleton_scripts_for_all_os() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();

        // Assert — sh scripts have shebang, ps1 scripts do not
        let pkg_dir = ctx.packages_dir().join("neovim");
        let sh_content = std::fs::read_to_string(pkg_dir.join("install.sh")).unwrap();
        assert!(sh_content.starts_with("#!/usr/bin/env sh"));

        let ps1_content = std::fs::read_to_string(pkg_dir.join("install.ps1")).unwrap();
        assert!(!ps1_content.contains("#!"));
        assert!(ps1_content.starts_with("# Generated by homeos"));
    }

    #[test]
    fn test_add_errors_when_package_already_exists() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_add_preserves_existing_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  ripgrep: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("ripgrep"));
        assert!(config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_add_errors_when_package_directory_exists() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"));
        assert!(err.contains("Remove it first"));
    }

    #[test]
    fn test_add_with_depends_on_stores_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &["git".to_string(), "curl".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_add_without_depends_on_has_empty_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].depends_on.is_empty());
    }

    #[test]
    fn test_add_with_depends_on_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let dependencies = vec!["git".to_string(), "curl".to_string()];

        // Act
        add(
            &ctx,
            "neovim",
            &dependencies,
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(config.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_add_with_script_aliases_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let script_aliases = BTreeMap::from([("update".to_string(), "install".to_string())]);

        // Act
        add(&ctx, "neovim", &[], &script_aliases, None, &BTreeMap::new()).unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(
            config.packages["neovim"].script_aliases,
            BTreeMap::from([("update".to_string(), "install".to_string())])
        );
    }

    #[test]
    fn test_add_with_empty_script_aliases_omits_field() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
        let yaml = std::fs::read_to_string(ctx.config_path()).unwrap();

        // Assert
        assert!(!yaml.contains("script_aliases"));
    }

    #[test]
    fn test_add_with_plugin_stores_plugin_name() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            Some("dnf"),
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].plugin, Some("dnf".to_string()));
    }

    #[test]
    fn test_add_with_params_stores_params() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());

        // Act
        let result = add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(
            config.packages["neovim"].params.get("name").unwrap(),
            "neovim.x86_64"
        );
    }

    #[test]
    fn test_add_without_plugin_has_no_plugin_or_params() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].plugin.is_none());
        assert!(config.packages["neovim"].params.is_empty());
    }

    #[test]
    fn test_add_with_plugin_and_params_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());
        params.insert("repo".to_string(), "extra".to_string());

        // Act
        add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params).unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(config.packages["neovim"].plugin, Some("dnf".to_string()));
        assert_eq!(config.packages["neovim"].params.len(), 2);
        assert_eq!(config.packages["neovim"].params["name"], "neovim.x86_64");
        assert_eq!(config.packages["neovim"].params["repo"], "extra");
    }

    #[test]
    fn test_add_with_plugin_generates_scripts_from_templates() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            plugin_dir.join(format!("install.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf install -y {{name}}\n",
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join(format!("update.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf update -y {{name}}\n",
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join(format!("uninstall.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf remove -y {{name}}\n",
        )
        .unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());

        // Act
        add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params).unwrap();

        // Assert
        let install_content = std::fs::read_to_string(
            ctx.packages_dir()
                .join("neovim")
                .join(format!("install.{ext}")),
        )
        .unwrap();
        assert!(install_content.contains("sudo dnf install -y neovim.x86_64"));
        let update_content = std::fs::read_to_string(
            ctx.packages_dir()
                .join("neovim")
                .join(format!("update.{ext}")),
        )
        .unwrap();
        assert!(update_content.contains("sudo dnf update -y neovim.x86_64"));
        let uninstall_content = std::fs::read_to_string(
            ctx.packages_dir()
                .join("neovim")
                .join(format!("uninstall.{ext}")),
        )
        .unwrap();
        assert!(uninstall_content.contains("sudo dnf remove -y neovim.x86_64"));
    }

    #[test]
    fn test_add_with_plugin_skips_missing_templates() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let ext = script_extension();
        // Only create install template, no update or uninstall
        std::fs::write(
            plugin_dir.join(format!("install.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf install -y {{name}}\n",
        )
        .unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());

        // Act
        add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params).unwrap();

        // Assert
        let pkg_dir = ctx.packages_dir().join("neovim");
        assert!(pkg_dir.join(format!("install.{ext}")).exists());
        assert!(!pkg_dir.join(format!("update.{ext}")).exists());
        assert!(!pkg_dir.join(format!("uninstall.{ext}")).exists());
    }

    #[test]
    fn test_add_with_plugin_errors_when_plugin_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            Some("nonexistent"),
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Plugin 'nonexistent' not found"));
    }

    #[test]
    fn test_add_with_plugin_errors_on_missing_required_params() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "params:\n  - name\n  - repo\n",
        )
        .unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &[],
            &BTreeMap::new(),
            Some("dnf"),
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Missing required plugin parameters"));
        assert!(err.contains("name"));
        assert!(err.contains("repo"));
    }

    #[test]
    fn test_add_with_plugin_replaces_multiple_params() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            plugin_dir.join(format!("install.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf install -y {{name}} --repo={{repo}}\n",
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "params:\n  - name\n  - repo\n",
        )
        .unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());
        params.insert("repo".to_string(), "extra".to_string());

        // Act
        add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params).unwrap();

        // Assert
        let content = std::fs::read_to_string(
            ctx.packages_dir()
                .join("neovim")
                .join(format!("install.{ext}")),
        )
        .unwrap();
        assert!(content.contains("sudo dnf install -y neovim.x86_64 --repo=extra"));
    }

    #[test]
    fn test_add_with_plugin_errors_when_package_directory_exists() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            plugin_dir.join(format!("install.{ext}.tmpl")),
            "#!/usr/bin/env sh\nsudo dnf install -y {{name}}\n",
        )
        .unwrap();
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), "neovim.x86_64".to_string());

        // Act
        let result = add(&ctx, "neovim", &[], &BTreeMap::new(), Some("dnf"), &params);

        // Assert
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"));
        assert!(err.contains("Remove it first"));
    }

    #[test]
    fn test_add_with_plugin_no_plugin_yml_skips_validation() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("simple");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            plugin_dir.join(format!("install.{ext}.tmpl")),
            "#!/usr/bin/env sh\necho hello\n",
        )
        .unwrap();
        // No plugin.yml

        // Act
        let result = add(
            &ctx,
            "mypkg",
            &[],
            &BTreeMap::new(),
            Some("simple"),
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let content = std::fs::read_to_string(
            ctx.packages_dir()
                .join("mypkg")
                .join(format!("install.{ext}")),
        )
        .unwrap();
        assert!(content.contains("echo hello"));
    }

    #[test]
    fn test_add_with_plugin_no_templates_creates_no_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("empty");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        // No template files

        // Act
        let result = add(
            &ctx,
            "mypkg",
            &[],
            &BTreeMap::new(),
            Some("empty"),
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let pkg_dir = ctx.packages_dir().join("mypkg");
        let ext = script_extension();
        assert!(!pkg_dir.join(format!("install.{ext}")).exists());
        assert!(!pkg_dir.join(format!("update.{ext}")).exists());
        assert!(!pkg_dir.join(format!("uninstall.{ext}")).exists());
    }

    #[test]
    fn test_remove_deletes_config_entry() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

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
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["nonexistent".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

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
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

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
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

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
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_remove_last_package_leaves_empty_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_remove_rejects_package_depended_on_by_others() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  git: {}\n  neovim:\n    depends_on:\n      - git\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, &["git".to_string()], false, &mut reader, &mut output);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot remove package 'git'"));
        assert!(err.contains("depended on by"));
        assert!(err.contains("neovim"));
        // Config should be unchanged
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("git"));
    }

    #[test]
    fn test_remove_rejects_package_depended_on_by_multiple() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  git: {}\n  neovim:\n    depends_on:\n      - git\n  ripgrep:\n    depends_on:\n      - git\n",
        );
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, &["git".to_string()], false, &mut reader, &mut output);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("neovim"));
        assert!(err.contains("ripgrep"));
    }

    #[test]
    fn test_remove_allows_package_not_depended_on() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  git: {}\n  neovim:\n    depends_on:\n      - git\n  ripgrep: {}\n",
        );
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["ripgrep".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_multiple_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n  git: {}\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
        assert!(!config.packages.contains_key("ripgrep"));
        assert!(config.packages.contains_key("git"));
    }

    #[test]
    fn test_remove_multiple_stops_on_first_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string(), "nonexistent".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
        // Config should be unchanged — no packages removed
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_remove_multiple_stops_on_installed() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("currently installed")
        );
        // Config should be unchanged
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_multiple_with_mutual_dependency() {
        // Arrange — neovim depends on git; removing both should succeed
        let (_tmp, ctx) =
            fixture("packages:\n  git: {}\n  neovim:\n    depends_on:\n      - git\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["git".to_string(), "neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_remove_purge_deletes_package_directory() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("install.sh"), "#!/bin/sh").unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        assert!(!pkg_dir.exists());
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Removed package 'neovim' and removed directory"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_purge_succeeds_when_directory_does_not_exist() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        // No package directory created
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Removed package 'neovim'"));
        assert!(!output_str.contains("removed directory"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_remove_without_purge_preserves_package_directory() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("install.sh"), "#!/bin/sh").unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        assert!(pkg_dir.exists());
        assert!(pkg_dir.join("install.sh").exists());
    }

    #[test]
    fn test_remove_purge_multiple_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n  git: {}\n");
        let neovim_dir = ctx.packages_dir().join("neovim");
        let ripgrep_dir = ctx.packages_dir().join("ripgrep");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::create_dir_all(&ripgrep_dir).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        assert!(!neovim_dir.exists());
        assert!(!ripgrep_dir.exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("git"));
    }

    #[test]
    fn test_remove_shows_confirmation_prompt() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be removed from homeos.yml:"));
        assert!(written.contains("  neovim"));
        assert!(written.contains("Proceed? [y/N]"));
    }

    #[test]
    fn test_remove_declined_does_not_remove() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let mut reader = Cursor::new(b"n\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        // Config should be unchanged
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_purge_shows_directories_in_prompt() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be removed from homeos.yml:"));
        assert!(written.contains("The following directories will be deleted:"));
        assert!(written.contains(&neovim_dir.display().to_string()));
    }

    #[test]
    fn test_remove_purge_no_directory_section_when_dirs_missing() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        // No package directory created
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be removed from homeos.yml:"));
        assert!(!written.contains("The following directories will be deleted:"));
    }

    #[test]
    fn test_remove_purge_declined_preserves_directory() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let mut reader = Cursor::new(b"n\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            true,
            &mut reader,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        assert!(pkg_dir.exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_add_dep_adds_dependency_to_existing_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  git: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["git".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git"]);
    }

    #[test]
    fn test_add_dep_adds_multiple_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  git: {}\n  curl: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["git".to_string(), "curl".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_add_dep_skips_duplicate_dependency() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["git".to_string(), "curl".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_add_dep_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n");

        // Act
        let result = add_dep(&ctx, "nonexistent", &["git".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_add_dep_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = add_dep(&ctx, "neovim", &["git".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_add_dep_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  git: {}\n");

        // Act
        add_dep(&ctx, "neovim", &["git".to_string()]).unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(config.packages["neovim"].depends_on, vec!["git"]);
    }

    #[test]
    fn test_add_dep_appends_to_existing_dependencies() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["curl".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_remove_dep_removes_dependency_from_package() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    depends_on:\n      - git\n      - curl\n  git: {}\n  curl: {}\n",
        );

        // Act
        let result = remove_dep(&ctx, "neovim", &["git".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["curl"]);
    }

    #[test]
    fn test_remove_dep_removes_multiple_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    depends_on:\n      - git\n      - curl\n      - wget\n  git: {}\n  curl: {}\n  wget: {}\n",
        );

        // Act
        let result = remove_dep(&ctx, "neovim", &["git".to_string(), "wget".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["curl"]);
    }

    #[test]
    fn test_remove_dep_skips_nonexistent_dependency() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");

        // Act
        let result = remove_dep(&ctx, "neovim", &["curl".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git"]);
    }

    #[test]
    fn test_remove_dep_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n");

        // Act
        let result = remove_dep(&ctx, "nonexistent", &["git".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_dep_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = remove_dep(&ctx, "neovim", &["git".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_dep_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    depends_on:\n      - git\n      - curl\n  git: {}\n  curl: {}\n",
        );

        // Act
        remove_dep(&ctx, "neovim", &["git".to_string()]).unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(config.packages["neovim"].depends_on, vec!["curl"]);
    }

    #[test]
    fn test_remove_dep_removes_all_dependencies_clears_list() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n");

        // Act
        let result = remove_dep(&ctx, "neovim", &["git".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].depends_on.is_empty());
    }

    #[test]
    fn test_add_alias_adds_alias_to_existing_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = add_alias(
            &ctx,
            "neovim",
            &[("update".to_string(), "install".to_string())],
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
    }

    #[test]
    fn test_add_alias_adds_multiple_aliases() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = add_alias(
            &ctx,
            "neovim",
            &[
                ("update".to_string(), "install".to_string()),
                ("uninstall".to_string(), "install".to_string()),
            ],
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
        assert_eq!(
            config.packages["neovim"].script_aliases["uninstall"],
            "install"
        );
    }

    #[test]
    fn test_add_alias_skips_duplicate_alias() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");

        // Act
        let result = add_alias(
            &ctx,
            "neovim",
            &[
                ("update".to_string(), "install".to_string()),
                ("uninstall".to_string(), "install".to_string()),
            ],
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].script_aliases.len(), 2);
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
    }

    #[test]
    fn test_add_alias_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n");

        // Act
        let result = add_alias(
            &ctx,
            "nonexistent",
            &[("update".to_string(), "install".to_string())],
        );

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_add_alias_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = add_alias(
            &ctx,
            "neovim",
            &[("update".to_string(), "install".to_string())],
        );

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_add_alias_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        add_alias(
            &ctx,
            "neovim",
            &[("update".to_string(), "install".to_string())],
        )
        .unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
    }

    #[test]
    fn test_add_alias_appends_to_existing_aliases() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");

        // Act
        let result = add_alias(
            &ctx,
            "neovim",
            &[("uninstall".to_string(), "install".to_string())],
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].script_aliases.len(), 2);
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
        assert_eq!(
            config.packages["neovim"].script_aliases["uninstall"],
            "install"
        );
    }

    #[test]
    fn test_remove_alias_removes_alias_from_package() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    script_aliases:\n      update: install\n      uninstall: install\n",
        );

        // Act
        let result = remove_alias(&ctx, "neovim", &["update".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].script_aliases.len(), 1);
        assert_eq!(
            config.packages["neovim"].script_aliases["uninstall"],
            "install"
        );
    }

    #[test]
    fn test_remove_alias_removes_multiple_aliases() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    script_aliases:\n      update: install\n      uninstall: install\n",
        );

        // Act
        let result = remove_alias(
            &ctx,
            "neovim",
            &["update".to_string(), "uninstall".to_string()],
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].script_aliases.is_empty());
    }

    #[test]
    fn test_remove_alias_skips_nonexistent_alias() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");

        // Act
        let result = remove_alias(&ctx, "neovim", &["uninstall".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
    }

    #[test]
    fn test_remove_alias_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n");

        // Act
        let result = remove_alias(&ctx, "nonexistent", &["update".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_alias_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = remove_alias(&ctx, "neovim", &["update".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_alias_persists_after_reload() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    script_aliases:\n      update: install\n      uninstall: install\n",
        );

        // Act
        remove_alias(&ctx, "neovim", &["update".to_string()]).unwrap();
        let config = Config::load(&ctx.config_path()).unwrap();

        // Assert
        assert_eq!(config.packages["neovim"].script_aliases.len(), 1);
        assert_eq!(
            config.packages["neovim"].script_aliases["uninstall"],
            "install"
        );
    }

    #[test]
    fn test_remove_alias_removes_all_aliases_clears_map() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");

        // Act
        let result = remove_alias(&ctx, "neovim", &["update".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].script_aliases.is_empty());
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_enable_preserves_other_fields() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    script_aliases:\n      update: install\n    enabled: false\n",
        );

        // Act
        let result = enable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
    }

    #[test]
    fn test_enable_multiple_packages() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    enabled: false\n  ripgrep:\n    enabled: false\n");

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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep:\n    enabled: false\n");

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
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n");

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
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_preserves_other_fields() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");

        // Act
        let result = disable(&ctx, &["neovim".to_string()]);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
        assert_eq!(
            config.packages["neovim"].script_aliases["update"],
            "install"
        );
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
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n");

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
    fn test_cat_displays_all_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        for ext in all_script_extensions() {
            std::fs::write(
                pkg_dir.join(format!("install.{ext}")),
                format!("echo install-{ext}\n"),
            )
            .unwrap();
            std::fs::write(
                pkg_dir.join(format!("update.{ext}")),
                format!("echo update-{ext}\n"),
            )
            .unwrap();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("echo uninstall-{ext}\n"),
            )
            .unwrap();
        }
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        for ext in all_script_extensions() {
            assert!(written.contains(&format!("=== install.{ext} ===")));
            assert!(written.contains(&format!("echo install-{ext}")));
            assert!(written.contains(&format!("=== update.{ext} ===")));
            assert!(written.contains(&format!("echo update-{ext}")));
            assert!(written.contains(&format!("=== uninstall.{ext} ===")));
            assert!(written.contains(&format!("echo uninstall-{ext}")));
        }
    }

    #[test]
    fn test_cat_shows_not_found_for_missing_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("install.sh"),
            "#!/usr/bin/env sh\necho install\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("=== install.sh ==="));
        assert!(written.contains("echo install"));
        assert!(written.contains("=== install.ps1 ===\n(not found)"));
        assert!(written.contains("=== update.sh ===\n(not found)"));
        assert!(written.contains("=== update.ps1 ===\n(not found)"));
        assert!(written.contains("=== uninstall.sh ===\n(not found)"));
        assert!(written.contains("=== uninstall.ps1 ===\n(not found)"));
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
        for ext in all_script_extensions() {
            assert!(written.contains(&format!("=== install.{ext} ===\n(not found)")));
            assert!(written.contains(&format!("=== update.{ext} ===\n(not found)")));
            assert!(written.contains(&format!("=== uninstall.{ext} ===\n(not found)")));
        }
    }

    #[test]
    fn test_cat_displays_both_sh_and_ps1_in_order() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("install.sh"), "echo install-sh\n").unwrap();
        std::fs::write(pkg_dir.join("install.ps1"), "echo install-ps1\n").unwrap();
        std::fs::write(pkg_dir.join("update.sh"), "echo update-sh\n").unwrap();
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        let sh_pos = written.find("=== install.sh ===").unwrap();
        let ps1_pos = written.find("=== install.ps1 ===").unwrap();
        let update_sh_pos = written.find("=== update.sh ===").unwrap();
        let update_ps1_pos = written.find("=== update.ps1 ===").unwrap();
        // sh comes before ps1 within each action
        assert!(sh_pos < ps1_pos);
        // install group comes before update group
        assert!(ps1_pos < update_sh_pos);
        assert!(update_sh_pos < update_ps1_pos);
        // missing scripts show (not found)
        assert!(written.contains("=== update.ps1 ===\n(not found)"));
        assert!(written.contains("=== uninstall.sh ===\n(not found)"));
        assert!(written.contains("=== uninstall.ps1 ===\n(not found)"));
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_err());
    }

    // --- cd tests ---

    #[test]
    fn test_cd_resolve_target_returns_packages_dir_without_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = resolve_cd_target(&ctx, None).unwrap();

        // Assert
        assert_eq!(result, ctx.packages_dir());
    }

    #[test]
    fn test_cd_resolve_target_returns_package_dir_with_package() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir().join("neovim")).unwrap();

        // Act
        let result = resolve_cd_target(&ctx, Some("neovim")).unwrap();

        // Assert
        assert_eq!(result, ctx.packages_dir().join("neovim"));
    }

    #[test]
    fn test_cd_resolve_target_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = resolve_cd_target(&ctx, Some("nonexistent"));

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_cd_resolve_target_errors_when_package_dir_missing() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        // Package is in config but directory doesn't exist

        // Act
        let result = resolve_cd_target(&ctx, Some("neovim"));

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Directory not found")
        );
    }

    #[test]
    fn test_cd_resolve_target_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()), "default".to_string());

        // Act
        let result = resolve_cd_target(&ctx, None);

        // Assert
        assert!(result.is_err());
    }
}
