use crate::config::{Config, PackageConfig, PluginManifest};
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::output::OutputFormat;
use crate::plan::prompt_confirm;
use crate::state::State;
use crate::topo::topological_sort;
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

    match ctx.output_format() {
        OutputFormat::Json => list_json(writer, &config, &installed_packages),
        OutputFormat::Text => list_text(writer, &config, &installed_packages),
    }
}

fn list_json<W: Write>(
    writer: &mut W,
    config: &Config,
    installed_packages: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<serde_json::Value> = config
        .packages
        .iter()
        .map(|(name, pkg)| {
            let installed = installed_packages.contains(name);
            serde_json::json!({
                "name": name,
                "enabled": pkg.enabled,
                "installed": installed,
                "depends_on": pkg.depends_on,
            })
        })
        .collect();
    writeln!(writer, "{}", serde_json::Value::Array(rows))?;
    Ok(())
}

fn list_text<W: Write>(
    writer: &mut W,
    config: &Config,
    installed_packages: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let name_width = config
        .packages
        .keys()
        .map(|n| n.len())
        .max()
        .unwrap_or(0)
        .max(7); // "Package" header length

    let deps_header = "Dependencies";
    let installed_header = "Installed";
    let installed_width = installed_header.len();

    let rows: Vec<(String, bool, bool, String)> = config
        .packages
        .iter()
        .map(|(name, pkg)| {
            let installed = installed_packages.contains(name);
            let deps = if pkg.depends_on.is_empty() {
                "-".to_string()
            } else {
                pkg.depends_on.join(", ")
            };
            (name.clone(), pkg.enabled, installed, deps)
        })
        .collect();

    let deps_width = rows
        .iter()
        .map(|(_, _, _, d)| d.len())
        .max()
        .unwrap_or(0)
        .max(deps_header.len());

    writeln!(
        writer,
        "{:<name_width$}  {:<7}  {:<installed_width$}  {:<deps_width$}",
        "Package", "Enabled", installed_header, deps_header
    )?;
    writeln!(
        writer,
        "{:<name_width$}  {:<7}  {:<installed_width$}  {:<deps_width$}",
        "-".repeat(name_width),
        "-------",
        "-".repeat(installed_width),
        "-".repeat(deps_width)
    )?;

    for (name, enabled, installed, deps) in &rows {
        let enabled_str = if *enabled { "yes" } else { "no" };
        let installed_str = if *installed { "yes" } else { "no" };
        writeln!(
            writer,
            "{:<name_width$}  {:<7}  {:<installed_width$}  {}",
            name, enabled_str, installed_str, deps
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
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Package '{package}' already exists"),
        )
        .into());
    }

    for dep in depends_on {
        if !config.packages.contains_key(dep.as_str()) {
            return Err(HomeosError::new(
                reasons::DEPENDENCY_NOT_FOUND,
                format!("Dependency '{dep}' not found"),
            )
            .into());
        }
    }

    // Check for circular dependencies by simulating the addition
    if !depends_on.is_empty() {
        let mut test_config = config.clone();
        test_config.packages.insert(
            package.to_string(),
            PackageConfig {
                depends_on: depends_on.to_vec(),
                ..Default::default()
            },
        );
        let all_packages: Vec<String> = test_config.packages.keys().cloned().collect();
        let topo_result = topological_sort(&test_config, &all_packages)?;
        if !topo_result.cycle.is_empty() {
            return Err(HomeosError::new(
                reasons::CIRCULAR_DEPENDENCY,
                format!(
                    "Circular dependency detected among packages: {}",
                    topo_result.cycle.join(", ")
                ),
            )
            .into());
        }
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
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Package directory '{package}' already exists. Remove it first to re-create."),
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
        return Err(HomeosError::new(
            reasons::PLUGIN_NOT_FOUND,
            format!(
                "Plugin '{plugin_name}' not found. Add it first with: homeos plugin add {plugin_name}"
            ),
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
            return Err(HomeosError::new(
                reasons::VALIDATION_ERROR,
                format!("Missing required plugin parameters: {list}"),
            )
            .into());
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
            return Err(HomeosError::new(
                reasons::PACKAGE_NOT_FOUND,
                format!("Package '{package}' not found"),
            )
            .into());
        }
    }

    let state_path = ctx.state_path();
    if state_path.exists() {
        let state = State::load(&state_path)?;
        for package in packages {
            if state.installed.contains(package) {
                return Err(HomeosError::new(
                    reasons::PACKAGE_INSTALLED,
                    format!(
                        "Package '{package}' is currently installed. Uninstall it first with: homeos package uninstall {package}"
                    ),
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
            return Err(HomeosError::new(
                reasons::DEPENDENT_EXISTS,
                format!("Cannot remove package '{package}' because it is depended on by: {list}"),
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

    if !ctx.yes() && !prompt_confirm(reader, writer) {
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

pub fn rename(ctx: &Context, old: &str, new: &str) -> Result<(), Box<dyn std::error::Error>> {
    rename_to(ctx, old, new, &mut std::io::stdout())
}

fn rename_to<W: Write>(
    ctx: &Context,
    old: &str,
    new: &str,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(old) {
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{old}' not found"),
        )
        .into());
    }
    if config.packages.contains_key(new) {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Package '{new}' already exists"),
        )
        .into());
    }

    let pkg_config = config.packages.remove(old).unwrap();
    config.packages.insert(new.to_string(), pkg_config);

    let mut updated_dependents: Vec<String> = Vec::new();
    for (name, pkg) in config.packages.iter_mut() {
        if let Some(pos) = pkg.depends_on.iter().position(|d| d == old) {
            pkg.depends_on[pos] = new.to_string();
            updated_dependents.push(name.clone());
        }
    }

    config.save(&ctx.config_path())?;

    let old_dir = ctx.packages_dir().join(old);
    let new_dir = ctx.packages_dir().join(new);
    if old_dir.exists() {
        std::fs::rename(&old_dir, &new_dir)?;
    }

    let state_path = ctx.state_path();
    if state_path.exists() {
        let mut state = State::load(&state_path)?;
        if let Some(pos) = state.installed.iter().position(|p| p == old) {
            state.installed[pos] = new.to_string();
            state.save(&state_path)?;
        }
    }

    writeln!(writer, "Renamed package '{old}' to '{new}'")?;
    for dependent in &updated_dependents {
        writeln!(writer, "Updated '{dependent}' dependency: {old} → {new}")?;
    }

    Ok(())
}

pub fn add_dep(
    ctx: &Context,
    package: &str,
    dependencies: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    add_dep_to(ctx, package, dependencies, &mut std::io::stdout())
}

fn add_dep_to(
    ctx: &Context,
    package: &str,
    dependencies: &[String],
    writer: &mut impl std::io::Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
        .into());
    }

    for dependency in dependencies {
        if !config.packages.contains_key(dependency.as_str()) {
            return Err(HomeosError::new(
                reasons::DEPENDENCY_NOT_FOUND,
                format!("Dependency '{dependency}' not found"),
            )
            .into());
        }
    }

    // Check for circular dependencies by simulating the addition
    {
        let mut test_config = config.clone();
        let pkg = test_config.packages.get_mut(package).unwrap();
        for dependency in dependencies {
            if !pkg.depends_on.contains(dependency) {
                pkg.depends_on.push(dependency.clone());
            }
        }
        let all_packages: Vec<String> = test_config.packages.keys().cloned().collect();
        let topo_result = topological_sort(&test_config, &all_packages)?;
        if !topo_result.cycle.is_empty() {
            return Err(HomeosError::new(
                reasons::CIRCULAR_DEPENDENCY,
                format!(
                    "Circular dependency detected among packages: {}",
                    topo_result.cycle.join(", ")
                ),
            )
            .into());
        }
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for dependency in dependencies {
        if pkg.depends_on.contains(dependency) {
            writeln!(
                writer,
                "Package '{package}' already depends on '{dependency}'"
            )?;
        } else {
            pkg.depends_on.push(dependency.clone());
            writeln!(writer, "Package '{package}' now depends on '{dependency}'")?;
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
    remove_dep_to(ctx, package, dependencies, &mut std::io::stdout())
}

fn remove_dep_to(
    ctx: &Context,
    package: &str,
    dependencies: &[String],
    writer: &mut impl std::io::Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.packages.contains_key(package) {
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
        .into());
    }

    let pkg = config.packages.get_mut(package).unwrap();

    for dependency in dependencies {
        if let Some(pos) = pkg.depends_on.iter().position(|d| d == dependency) {
            pkg.depends_on.remove(pos);
            writeln!(
                writer,
                "Package '{package}' no longer depends on '{dependency}'"
            )?;
        } else {
            writeln!(
                writer,
                "Package '{package}' does not depend on '{dependency}'"
            )?;
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
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
        .into());
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
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
        .into());
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
        let pkg = config.packages.get_mut(package.as_str()).ok_or_else(|| {
            HomeosError::new(
                reasons::PACKAGE_NOT_FOUND,
                format!("Package '{package}' not found"),
            )
        })?;

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
        let pkg = config.packages.get_mut(package.as_str()).ok_or_else(|| {
            HomeosError::new(
                reasons::PACKAGE_NOT_FOUND,
                format!("Package '{package}' not found"),
            )
        })?;

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

pub fn info(ctx: &Context, package: &str) -> Result<(), Box<dyn std::error::Error>> {
    info_to(ctx, package, &mut std::io::stdout())
}

fn info_to<W: Write>(
    ctx: &Context,
    package: &str,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let pkg = config.packages.get(package).ok_or_else(|| {
        HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
    })?;

    let state_path = ctx.state_path();
    let installed = if state_path.exists() {
        State::load(&state_path)?
            .installed
            .contains(&package.to_string())
    } else {
        false
    };

    let dependents: Vec<String> = config
        .packages
        .iter()
        .filter(|(_, p)| p.depends_on.contains(&package.to_string()))
        .map(|(name, _)| name.clone())
        .collect();

    let pkg_dir = ctx.packages_dir().join(package);

    match ctx.output_format() {
        OutputFormat::Json => info_json(writer, package, pkg, installed, &dependents, &pkg_dir),
        OutputFormat::Text => info_text(writer, package, pkg, installed, &dependents, &pkg_dir),
    }
}

fn info_json<W: Write>(
    writer: &mut W,
    package: &str,
    pkg: &PackageConfig,
    installed: bool,
    dependents: &[String],
    pkg_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let actions = ["install", "update", "uninstall"];
    let extensions = super::all_script_extensions();

    let scripts: Vec<serde_json::Value> = actions
        .iter()
        .flat_map(|action| {
            extensions.iter().map(move |ext| {
                let filename = format!("{action}.{ext}");
                let script_path = pkg_dir.join(&filename);
                let path = if script_path.is_file() {
                    serde_json::Value::String(script_path.display().to_string())
                } else {
                    serde_json::Value::Null
                };
                serde_json::json!({
                    "filename": filename,
                    "path": path,
                })
            })
        })
        .collect();

    let value = serde_json::json!({
        "name": package,
        "enabled": pkg.enabled,
        "installed": installed,
        "plugin": pkg.plugin,
        "params": pkg.params,
        "depends_on": pkg.depends_on,
        "dependents": dependents,
        "script_aliases": pkg.script_aliases,
        "scripts": scripts,
    });
    writeln!(writer, "{value}")?;
    Ok(())
}

fn info_text<W: Write>(
    writer: &mut W,
    package: &str,
    pkg: &PackageConfig,
    installed: bool,
    dependents: &[String],
    pkg_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(writer, "Package: {package}")?;
    writeln!(
        writer,
        "Enabled: {}",
        if pkg.enabled { "yes" } else { "no" }
    )?;
    writeln!(
        writer,
        "Installed: {}",
        if installed { "yes" } else { "no" }
    )?;
    writeln!(writer, "Plugin: {}", pkg.plugin.as_deref().unwrap_or("-"))?;

    writeln!(writer, "Dependencies:")?;
    if pkg.depends_on.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for dep in &pkg.depends_on {
            writeln!(writer, "  {dep}")?;
        }
    }

    writeln!(writer, "Dependents:")?;
    if dependents.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for dep in dependents {
            writeln!(writer, "  {dep}")?;
        }
    }

    writeln!(writer, "Script aliases:")?;
    if pkg.script_aliases.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for (target, source) in &pkg.script_aliases {
            writeln!(writer, "  {target} → {source}")?;
        }
    }

    let actions = ["install", "update", "uninstall"];
    let extensions = super::all_script_extensions();

    writeln!(writer, "Scripts:")?;
    for action in &actions {
        for ext in extensions {
            let filename = format!("{action}.{ext}");
            let script_path = pkg_dir.join(&filename);
            if script_path.is_file() {
                writeln!(writer, "  {filename} ({})", script_path.display())?;
            } else {
                writeln!(writer, "  {filename} (not found)")?;
            }
        }
    }

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
        return Err(HomeosError::new(
            reasons::PACKAGE_NOT_FOUND,
            format!("Package '{package}' not found"),
        )
        .into());
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

pub fn cd(
    ctx: &Context,
    package: Option<&str>,
    print: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_cd_target(ctx, package)?;
    if print {
        return print_cd_path(&dir, ctx.output_format(), &mut std::io::stdout());
    }
    let shell = crate::commands::detect_shell();

    let status = std::process::Command::new(&shell)
        .current_dir(&dir)
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn print_cd_path<W: Write>(
    dir: &std::path::Path,
    format: OutputFormat,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            let value = serde_json::json!({ "path": dir.display().to_string() });
            writeln!(writer, "{value}")?;
        }
        OutputFormat::Text => {
            writeln!(writer, "{}", dir.display())?;
        }
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
                return Err(HomeosError::new(
                    reasons::PACKAGE_NOT_FOUND,
                    format!("Package '{pkg}' not found"),
                )
                .into());
            }
            ctx.packages_dir().join(pkg)
        }
        None => ctx.packages_dir(),
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
    use crate::error::reasons;
    use crate::state::State;
    use std::collections::BTreeMap;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(data_dir));

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
        assert!(text.contains("Dependencies"));
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
        assert!(text.contains("Dependencies"));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator only
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
        assert!(lines[2].contains("  yes  "));
        // ripgrep: enabled=yes, installed=no
        assert!(lines[3].contains("ripgrep"));
        assert!(lines[3].contains("  no   "));
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
        assert!(lines[2].contains("  no   "));
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
        assert!(lines[0].contains("Dependencies"));
        // Second line is separator
        assert!(lines[1].contains("-------"));
        assert!(lines[1].contains("------------"));
    }

    #[test]
    fn test_list_shows_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on: [bubblewrap, socat]\n  bubblewrap: {}\n  socat: {}\n",
        );
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // bubblewrap has no dependencies
        assert!(lines[2].contains("bubblewrap"));
        assert!(lines[2].ends_with("-"));
        // claude has dependencies
        assert!(lines[3].contains("claude"));
        assert!(lines[3].contains("bubblewrap, socat"));
        // socat has no dependencies
        assert!(lines[4].contains("socat"));
        assert!(lines[4].ends_with("-"));
    }

    #[test]
    fn test_list_dependencies_column_separator_matches_widest_value() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on: [bubblewrap, socat]\n  bubblewrap: {}\n  socat: {}\n",
        );
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        // The widest deps value is "bubblewrap, socat" (17 chars), wider than the
        // "Dependencies" header (12 chars), so the separator must be 17 dashes.
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let deps_separator = lines[1].rsplit("  ").next().unwrap();
        assert_eq!(deps_separator, "-".repeat(17));
    }

    #[test]
    fn test_list_dependencies_column_separator_matches_header_when_values_shorter() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        // All values are "-" (1 char), narrower than the "Dependencies" header
        // (12 chars), so the separator falls back to header width: 12 dashes.
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let deps_separator = lines[1].rsplit("  ").next().unwrap();
        assert_eq!(deps_separator, "-".repeat(12));
    }

    #[test]
    fn test_list_shows_dash_for_no_dependencies() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[2].contains("neovim"));
        assert!(lines[2].ends_with("-"));
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n  curl: {}\n");
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
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n  curl: {}\n");
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
            "description: DNF plugin\nparams:\n  - name\n  - repo\n",
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
            "description: DNF plugin\nparams:\n  - name\n  - repo\n",
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
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
    fn test_remove_yes_skips_prompt_and_removes() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ctx = ctx.with_yes(true);
        let mut reader = Cursor::new(b"");
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
        assert!(!written.contains("Proceed? [y/N]"));
        assert!(!written.contains("Aborted."));
        assert!(written.contains("Removed package 'neovim'"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("ripgrep"));
    }

    #[test]
    fn test_remove_yes_with_purge_skips_prompt_and_deletes_directory() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ctx = ctx.with_yes(true);
        let mut reader = Cursor::new(b"");
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
        assert!(!written.contains("Proceed? [y/N]"));
        assert!(written.contains("Removed package 'neovim' and removed directory"));
        assert!(!pkg_dir.exists());
    }

    #[test]
    fn test_rename_updates_config_entry_key() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = rename_to(&ctx, "neovim", "nvim", &mut output);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
        assert!(config.packages.contains_key("nvim"));
    }

    #[test]
    fn test_rename_preserves_package_config_fields() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    enabled: false\n    script_aliases:\n      update: install\n",
        );
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        let pkg = &config.packages["nvim"];
        assert!(!pkg.enabled);
        assert_eq!(pkg.script_aliases["update"], "install");
    }

    #[test]
    fn test_rename_renames_package_directory_on_disk() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let old_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("install.sh"), "echo hello").unwrap();
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let new_dir = ctx.packages_dir().join("nvim");
        assert!(!ctx.packages_dir().join("neovim").exists());
        assert!(new_dir.is_dir());
        let content = std::fs::read_to_string(new_dir.join("install.sh")).unwrap();
        assert_eq!(content, "echo hello");
    }

    #[test]
    fn test_rename_updates_state_when_installed() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let state = State {
            installed: vec!["neovim".to_string(), "git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["nvim".to_string(), "git".to_string()]);
    }

    #[test]
    fn test_rename_leaves_state_unchanged_when_not_installed() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let state = State {
            installed: vec!["git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["git".to_string()]);
    }

    #[test]
    fn test_rename_updates_depends_on_references_in_other_packages() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on:\n      - neovim\n      - git\n  neovim: {}\n  git: {}\n",
        );
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["claude"].depends_on, vec!["nvim", "git"]);
    }

    #[test]
    fn test_rename_prints_updated_dependency_messages() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  a:\n    depends_on:\n      - neovim\n  b:\n    depends_on:\n      - neovim\n  neovim: {}\n",
        );
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Updated 'a' dependency: neovim → nvim"));
        assert!(text.contains("Updated 'b' dependency: neovim → nvim"));
    }

    #[test]
    fn test_rename_errors_when_old_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = rename_to(&ctx, "nonexistent", "nvim", &mut output);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(err, "Package 'nonexistent' not found");
    }

    #[test]
    fn test_rename_errors_when_new_already_exists() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  nvim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = rename_to(&ctx, "neovim", "nvim", &mut output);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(err, "Package 'nvim' already exists");
    }

    #[test]
    fn test_rename_prints_success_message() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        rename_to(&ctx, "neovim", "nvim", &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Renamed package 'neovim' to 'nvim'"));
    }

    #[test]
    fn test_rename_succeeds_when_package_directory_does_not_exist() {
        // Arrange — package exists in config but no directory on disk
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = rename_to(&ctx, "neovim", "nvim", &mut output);

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.contains_key("nvim"));
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
    fn test_add_dep_errors_when_dependency_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["nonexistent".to_string()]);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependency 'nonexistent' not found"
        );
    }

    #[test]
    fn test_add_dep_errors_when_one_of_multiple_dependencies_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  git: {}\n");

        // Act
        let result = add_dep(&ctx, "neovim", &["git".to_string(), "missing".to_string()]);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependency 'missing' not found"
        );
        // Verify no partial changes were saved
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].depends_on.is_empty());
    }

    #[test]
    fn test_add_with_depends_on_errors_when_dependency_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &["nonexistent".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dependency 'nonexistent' not found"
        );
        // Verify the package was not added
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages.contains_key("neovim"));
    }

    #[test]
    fn test_add_with_valid_depends_on_succeeds() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  git: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "neovim",
            &["git".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages["neovim"].depends_on, vec!["git"]);
    }

    #[test]
    fn test_add_dep_errors_on_circular_dependency() {
        // Arrange — a depends on b, try to add b depends on a
        let (_tmp, ctx) = fixture("packages:\n  a:\n    depends_on: [b]\n  b: {}\n");

        // Act
        let result = add_dep(&ctx, "b", &["a".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_add_dep_errors_on_self_dependency() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  a: {}\n");

        // Act
        let result = add_dep(&ctx, "a", &["a".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_add_dep_errors_on_transitive_circular_dependency() {
        // Arrange — a -> b -> c, try to add c -> a
        let (_tmp, ctx) =
            fixture("packages:\n  a:\n    depends_on: [b]\n  b:\n    depends_on: [c]\n  c: {}\n");

        // Act
        let result = add_dep(&ctx, "c", &["a".to_string()]);

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_add_dep_no_changes_on_circular_dependency_error() {
        // Arrange — a depends on b, try to add b depends on a
        let (_tmp, ctx) = fixture("packages:\n  a:\n    depends_on: [b]\n  b: {}\n");

        // Act
        let _ = add_dep(&ctx, "b", &["a".to_string()]);

        // Assert — config should be unchanged
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["b"].depends_on.is_empty());
    }

    #[test]
    fn test_add_dep_outputs_now_depends_on_message() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  git: {}\n");
        let mut output = Vec::new();

        // Act
        add_dep_to(&ctx, "neovim", &["git".to_string()], &mut output).unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(output_str, "Package 'neovim' now depends on 'git'\n");
    }

    #[test]
    fn test_add_dep_outputs_already_depends_on_message() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n");
        let mut output = Vec::new();

        // Act
        add_dep_to(&ctx, "neovim", &["git".to_string()], &mut output).unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(output_str, "Package 'neovim' already depends on 'git'\n");
    }

    #[test]
    fn test_add_dep_outputs_mixed_messages_for_multiple_deps() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");
        let mut output = Vec::new();

        // Act
        add_dep_to(
            &ctx,
            "neovim",
            &["git".to_string(), "curl".to_string()],
            &mut output,
        )
        .unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Package 'neovim' already depends on 'git'"));
        assert!(output_str.contains("Package 'neovim' now depends on 'curl'"));
    }

    #[test]
    fn test_add_with_depends_on_errors_on_circular_dependency() {
        // Arrange — b depends on (future) a; add a with depends_on b creates a -> b -> a cycle
        let (_tmp, ctx) = fixture("packages:\n  b:\n    depends_on: [a]\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "a",
            &["b".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
    fn test_remove_dep_outputs_no_longer_depends_on_message() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n");
        let mut output = Vec::new();

        // Act
        remove_dep_to(&ctx, "neovim", &["git".to_string()], &mut output).unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Package 'neovim' no longer depends on 'git'"));
    }

    #[test]
    fn test_remove_dep_outputs_does_not_depend_on_message() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");
        let mut output = Vec::new();

        // Act
        remove_dep_to(&ctx, "neovim", &["curl".to_string()], &mut output).unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Package 'neovim' does not depend on 'curl'"));
    }

    #[test]
    fn test_remove_dep_outputs_mixed_messages_for_multiple_deps() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n  curl: {}\n");
        let mut output = Vec::new();

        // Act
        remove_dep_to(
            &ctx,
            "neovim",
            &["git".to_string(), "curl".to_string()],
            &mut output,
        )
        .unwrap();

        // Assert
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Package 'neovim' no longer depends on 'git'"));
        assert!(output_str.contains("Package 'neovim' does not depend on 'curl'"));
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

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
    fn test_info_displays_package_details() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on: [bubblewrap, socat]\n    script_aliases:\n      update: install\n  bubblewrap: {}\n  socat: {}\n",
        );
        let state = State {
            installed: vec!["claude".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "claude", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Package: claude"));
        assert!(text.contains("Enabled: yes"));
        assert!(text.contains("Installed: yes"));
        assert!(text.contains("Plugin: -"));
        assert!(text.contains("Dependencies:"));
        assert!(text.contains("  bubblewrap"));
        assert!(text.contains("  socat"));
        assert!(text.contains("Dependents:"));
        assert!(text.contains("  (none)"));
        assert!(text.contains("Script aliases:"));
        assert!(text.contains("  update → install"));
        assert!(text.contains("Scripts:"));
        assert!(text.contains("  install.sh (not found)"));
        assert!(text.contains("  install.ps1 (not found)"));
    }

    #[test]
    fn test_info_shows_dependents() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  app:\n    depends_on: [lib]\n  lib: {}\n");
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "lib", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Dependents:"));
        assert!(text.contains("  app"));
    }

    #[test]
    fn test_info_disabled_and_not_installed() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  ollama:\n    enabled: false\n");
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "ollama", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Enabled: no"));
        assert!(text.contains("Installed: no"));
        assert!(text.contains("Dependencies:\n  (none)"));
        assert!(text.contains("Dependents:\n  (none)"));
        assert!(text.contains("Script aliases:\n  (none)"));
        assert!(text.contains("Scripts:"));
    }

    #[test]
    fn test_info_shows_plugin() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    plugin: dnf\n    params:\n      name: neovim.x86_64\n",
        );
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Plugin: dnf"));
    }

    #[test]
    fn test_info_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "nonexistent", &mut output);

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Package 'nonexistent' not found")
        );
    }

    #[test]
    fn test_info_errors_when_not_initialized() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_info_scripts_shows_existing_with_full_path() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("install.sh"), "#!/usr/bin/env sh\n").unwrap();
        std::fs::write(pkg_dir.join("update.ps1"), "# powershell\n").unwrap();
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Scripts:"));
        let install_sh_path = pkg_dir.join("install.sh");
        assert!(text.contains(&format!("  install.sh ({})", install_sh_path.display())));
        assert!(text.contains("  install.ps1 (not found)"));
        assert!(text.contains("  update.sh (not found)"));
        let update_ps1_path = pkg_dir.join("update.ps1");
        assert!(text.contains(&format!("  update.ps1 ({})", update_ps1_path.display())));
        assert!(text.contains("  uninstall.sh (not found)"));
        assert!(text.contains("  uninstall.ps1 (not found)"));
    }

    #[test]
    fn test_info_scripts_shows_all_not_found_when_no_scripts_exist() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Scripts:"));
        assert!(text.contains("  install.sh (not found)"));
        assert!(text.contains("  install.ps1 (not found)"));
        assert!(text.contains("  update.sh (not found)"));
        assert!(text.contains("  update.ps1 (not found)"));
        assert!(text.contains("  uninstall.sh (not found)"));
        assert!(text.contains("  uninstall.ps1 (not found)"));
    }

    #[test]
    fn test_info_json_emits_object_with_all_fields() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on: [bubblewrap, socat]\n    script_aliases:\n      update: install\n  bubblewrap: {}\n  socat: {}\n",
        );
        let state = State {
            installed: vec!["claude".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "claude", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["name"], "claude");
        assert_eq!(value["enabled"], true);
        assert_eq!(value["installed"], true);
        assert_eq!(value["plugin"], serde_json::Value::Null);
        assert_eq!(value["params"], serde_json::json!({}));
        assert_eq!(
            value["depends_on"],
            serde_json::json!(["bubblewrap", "socat"])
        );
        assert_eq!(value["dependents"], serde_json::json!([]));
        assert_eq!(
            value["script_aliases"],
            serde_json::json!({"update": "install"})
        );
        assert!(value["scripts"].is_array());
    }

    #[test]
    fn test_info_json_emits_null_plugin_when_absent() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["plugin"], serde_json::Value::Null);
    }

    #[test]
    fn test_info_json_emits_plugin_and_params_when_present() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    plugin: dnf\n    params:\n      name: neovim.x86_64\n",
        );
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["plugin"], "dnf");
        assert_eq!(
            value["params"],
            serde_json::json!({"name": "neovim.x86_64"})
        );
    }

    #[test]
    fn test_info_json_emits_dependents() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  app:\n    depends_on: [lib]\n  lib: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "lib", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["dependents"], serde_json::json!(["app"]));
    }

    #[test]
    fn test_info_json_scripts_have_path_or_null() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("install.sh"), "#!/usr/bin/env sh\n").unwrap();
        let install_sh_path = pkg_dir.join("install.sh");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let scripts = value["scripts"].as_array().unwrap();
        assert_eq!(scripts.len(), 6);
        let install_sh = scripts
            .iter()
            .find(|s| s["filename"] == "install.sh")
            .unwrap();
        assert_eq!(install_sh["path"], install_sh_path.display().to_string());
        let install_ps1 = scripts
            .iter()
            .find(|s| s["filename"] == "install.ps1")
            .unwrap();
        assert_eq!(install_ps1["path"], serde_json::Value::Null);
    }

    #[test]
    fn test_info_json_emits_single_line() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        let result = info_to(&ctx, "neovim", &mut output);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(output).unwrap();
        let non_trailing = text.trim_end_matches('\n');
        assert!(!non_trailing.contains('\n'));
        assert!(text.ends_with('\n'));
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
    fn test_cat_package_not_found_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut output = Vec::new();

        // Act
        let result = cat_to(&ctx, "nonexistent", &mut output);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::PACKAGE_NOT_FOUND);
    }

    #[test]
    fn test_add_already_exists_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
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
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::ALREADY_EXISTS);
    }

    #[test]
    fn test_add_dependency_not_found_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();

        // Act
        let result = add(
            &ctx,
            "ripgrep",
            &["missing".to_string()],
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
        );

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::DEPENDENCY_NOT_FOUND);
    }

    #[test]
    fn test_remove_dependent_exists_reason() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  base: {}\n  child:\n    depends_on:\n      - base\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let mut reader = Cursor::new("y\n");
        let mut writer = Vec::new();

        // Act
        let result = remove_to(&ctx, &["base".to_string()], false, &mut reader, &mut writer);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::DEPENDENT_EXISTS);
    }

    #[test]
    fn test_remove_package_installed_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut reader = Cursor::new("y\n");
        let mut writer = Vec::new();

        // Act
        let result = remove_to(
            &ctx,
            &["neovim".to_string()],
            false,
            &mut reader,
            &mut writer,
        );

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::PACKAGE_INSTALLED);
    }

    #[test]
    fn test_rename_target_already_exists_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  old: {}\n  new: {}\n");
        std::fs::create_dir_all(ctx.packages_dir()).unwrap();
        let mut writer = Vec::new();

        // Act
        let result = rename_to(&ctx, "old", "new", &mut writer);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::ALREADY_EXISTS);
    }

    #[test]
    fn test_enable_package_not_found_reason() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");

        // Act
        let result = enable(&ctx, &["nonexistent".to_string()]);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::PACKAGE_NOT_FOUND);
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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));

        // Act
        let result = resolve_cd_target(&ctx, None);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_cd_print_text_emits_packages_root() {
        // Arrange
        let dir = std::path::PathBuf::from("/home/user/.local/share/homeos/packages");
        let mut output = Vec::new();

        // Act
        print_cd_path(&dir, OutputFormat::Text, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "/home/user/.local/share/homeos/packages\n");
    }

    #[test]
    fn test_cd_print_text_emits_package_dir() {
        // Arrange
        let dir = std::path::PathBuf::from("/home/user/.local/share/homeos/packages/neovim");
        let mut output = Vec::new();

        // Act
        print_cd_path(&dir, OutputFormat::Text, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "/home/user/.local/share/homeos/packages/neovim\n");
    }

    #[test]
    fn test_cd_print_json_emits_object_with_path() {
        // Arrange
        let dir = std::path::PathBuf::from("/home/user/.local/share/homeos/packages/neovim");
        let mut output = Vec::new();

        // Act
        print_cd_path(&dir, OutputFormat::Json, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value["path"],
            "/home/user/.local/share/homeos/packages/neovim"
        );
    }

    #[test]
    fn test_cd_print_errors_when_package_not_found() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");

        // Act
        let result = cd(&ctx, Some("nonexistent"), true);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::PACKAGE_NOT_FOUND);
    }

    #[test]
    fn test_list_json_emits_array_of_objects() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert_eq!(array.len(), 2);
        // BTreeMap iterates alphabetically: neovim, ripgrep
        assert_eq!(array[0]["name"], "neovim");
        assert_eq!(array[1]["name"], "ripgrep");
    }

    #[test]
    fn test_list_json_emits_empty_array_when_no_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert!(array.is_empty());
    }

    #[test]
    fn test_list_json_enabled_field_is_boolean() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n");
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        assert_eq!(array[0]["name"], "neovim");
        assert_eq!(array[0]["enabled"], false);
        assert_eq!(array[1]["name"], "ripgrep");
        assert_eq!(array[1]["enabled"], true);
    }

    #[test]
    fn test_list_json_installed_field_reflects_state() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        assert_eq!(array[0]["name"], "neovim");
        assert_eq!(array[0]["installed"], true);
        assert_eq!(array[1]["name"], "ripgrep");
        assert_eq!(array[1]["installed"], false);
    }

    #[test]
    fn test_list_json_depends_on_field_is_array() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  claude:\n    depends_on: [bubblewrap, socat]\n  bubblewrap: {}\n  socat: {}\n",
        );
        let ctx = ctx.with_output_format(OutputFormat::Json);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        let claude = array
            .iter()
            .find(|v| v["name"] == "claude")
            .expect("claude entry");
        assert_eq!(
            claude["depends_on"],
            serde_json::json!(["bubblewrap", "socat"])
        );
        let bubblewrap = array
            .iter()
            .find(|v| v["name"] == "bubblewrap")
            .expect("bubblewrap entry");
        assert_eq!(bubblewrap["depends_on"], serde_json::json!([]));
    }
}
