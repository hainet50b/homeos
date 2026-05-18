use crate::config::{Config, PackageConfig};
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::plan::{Action, Plan, confirm_plan};
use crate::state::State;
use crate::topo::topological_sort;
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::Path;

pub fn apply(ctx: &Context, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    apply_to(
        ctx,
        dry_run,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub(crate) fn apply_to<R: BufRead, W: Write>(
    ctx: &Context,
    dry_run: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;
    let state_path = ctx.state_path();
    let installed = if state_path.exists() {
        State::load(&state_path)?.installed
    } else {
        Vec::new()
    };

    let mut to_install = Vec::new();
    let mut to_update = Vec::new();
    let mut disabled_packages = Vec::new();

    for (name, pkg) in &config.packages {
        if !pkg.enabled {
            disabled_packages.push(name.clone());
            continue;
        }
        if installed.contains(name) {
            to_update.push(name.clone());
        } else {
            to_install.push(name.clone());
        }
    }

    if to_install.is_empty() && to_update.is_empty() {
        let plan = Plan::build(
            &config,
            &disabled_packages,
            Action::Install,
            &installed,
            Some(&ctx.packages_dir()),
        )?;
        let display = plan.display();
        if !display.is_empty() {
            writeln!(writer, "{display}")?;
            writeln!(writer)?;
        }
        writeln!(writer, "Nothing to do.")?;
        return Ok(());
    }

    // Expand install dependencies — some deps may already be installed (update targets).
    // The notes from expand_dependencies are discarded; apply computes its own intra-set
    // requester annotations below (every package in apply is implicitly requested, so
    // expand_dependencies' "skip if explicitly requested" rule produces incomplete notes).
    //
    // Filter disabled packages out of `expanded_install` so they no longer reach
    // `install_names` / `Plan::build`. This leaves `disabled_packages` as the single
    // source of truth for the `(disabled)` skipped entries, avoiding a duplicate
    // skipped section when a dependency chain pulls a disabled package into the plan.
    let expanded_install: Vec<String> = if !to_install.is_empty() {
        expand_dependencies(&config, &to_install)
            .0
            .into_iter()
            .filter(|name| config.packages.get(name).is_none_or(|p| p.enabled))
            .collect()
    } else {
        Vec::new()
    };

    // Merge all packages (install + update + expanded deps) into a single set,
    // then topologically sort for unified dependency-ordered execution.
    let install_set: HashSet<&str> = expanded_install.iter().map(|s| s.as_str()).collect();
    let update_set: HashSet<&str> = to_update.iter().map(|s| s.as_str()).collect();
    let all_packages: Vec<String> = install_set
        .union(&update_set)
        .map(|s| s.to_string())
        .collect();
    let topo_result = topological_sort(&config, &all_packages)?;
    let ordered = topo_result.sorted;
    let cycle_packages = topo_result.cycle;

    // Compute intra-set "required by" notes. For each package in the merged ordered set,
    // find a direct requester among the other packages in the set; if multiple, pick the
    // first alphabetically for determinism.
    let mut intra_set_notes: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for name in &ordered {
        let mut requesters: Vec<&str> = Vec::new();
        for other in &ordered {
            if other == name {
                continue;
            }
            if let Some(other_cfg) = config.packages.get(other)
                && other_cfg.depends_on.contains(name)
            {
                requesters.push(other.as_str());
            }
        }
        if !requesters.is_empty() {
            requesters.sort();
            intra_set_notes.insert(name.clone(), format!("required by {}", requesters[0]));
        }
    }

    // Classify each package: if in state -> update, else -> install
    let installed_set: HashSet<&str> = installed.iter().map(|s| s.as_str()).collect();
    let mut ordered_actions: Vec<(String, Action)> = Vec::new();
    for name in &ordered {
        if installed_set.contains(name.as_str()) {
            ordered_actions.push((name.clone(), Action::Update));
        } else {
            ordered_actions.push((name.clone(), Action::Install));
        }
    }

    // Build plans for display (still separate for clear output)
    let install_names: Vec<String> = ordered_actions
        .iter()
        .filter(|(_, a)| *a == Action::Install)
        .map(|(n, _)| n.clone())
        .collect();
    let update_names: Vec<String> = ordered_actions
        .iter()
        .filter(|(_, a)| *a == Action::Update)
        .map(|(n, _)| n.clone())
        .collect();

    // install_plan absorbs disabled_packages (top-level disabled in config) and
    // cycle_packages so that all skipped reasons render under a single consolidated
    // skipped section via `display_skipped()`. Plan::build classifies disabled
    // packages as `disabled` (with plugin lookup), and dependents whose dep chain
    // is unavailable as `dependency_disabled` — both rendered together.
    let install_input: Vec<String> = install_names
        .iter()
        .chain(disabled_packages.iter())
        .cloned()
        .collect();
    let mut install_plan = if !install_input.is_empty() || !cycle_packages.is_empty() {
        let mut plan = Plan::build(
            &config,
            &install_input,
            Action::Install,
            &installed,
            Some(&ctx.packages_dir()),
        )?;
        plan.notes = intra_set_notes.clone();
        plan.circular_dependency = cycle_packages.clone();
        Some(plan)
    } else {
        None
    };
    let update_plan = if !update_names.is_empty() {
        let mut plan = Plan::build(
            &config,
            &update_names,
            Action::Update,
            &installed,
            Some(&ctx.packages_dir()),
        )?;
        plan.notes = intra_set_notes;
        Some(plan)
    } else {
        None
    };

    // Surface update-side `script_unmodified` entries through install_plan's
    // consolidated skipped section. update_plan can only contribute
    // script_unmodified to skipped in apply (its input is enabled+in_state, so
    // Action::Update never produces disabled/not_installed/dependency_disabled
    // entries), and only install_plan.display_skipped() is rendered, so without
    // this merge update-side unmodified scripts would be silently dropped.
    if let Some(ref update_p) = update_plan
        && !update_p.script_unmodified.is_empty()
    {
        let install_p = install_plan.get_or_insert_with(|| Plan {
            action: Action::Install,
            enabled: Vec::new(),
            disabled: Vec::new(),
            already_installed: Vec::new(),
            not_installed: Vec::new(),
            circular_dependency: Vec::new(),
            dependency_disabled: std::collections::BTreeMap::new(),
            script_unmodified: std::collections::BTreeMap::new(),
            plugins: std::collections::BTreeMap::new(),
            notes: std::collections::BTreeMap::new(),
        });
        for (name, script_name) in &update_p.script_unmodified {
            install_p
                .script_unmodified
                .insert(name.clone(), script_name.clone());
            if let Some(plugin) = update_p.plugins.get(name) {
                install_p.plugins.insert(name.clone(), plugin.clone());
            }
        }
    }

    // Display: enabled sections first (install, then update) so the order matches
    // README — installed → updated → skipped — followed by one consolidated skipped
    // section sourced from install_plan (which absorbs all skipped entries).
    if let Some(ref plan) = install_plan {
        let s = plan.display_enabled();
        if !s.is_empty() {
            writeln!(writer, "{s}")?;
        }
    }
    if let Some(ref plan) = update_plan {
        let s = plan.display_enabled();
        if !s.is_empty() {
            writeln!(writer, "{s}")?;
        }
    }
    if let Some(ref plan) = install_plan {
        let s = plan.display_skipped();
        if !s.is_empty() {
            writeln!(writer, "{s}")?;
        }
    }

    // If all packages were in cycle and nothing to install/update, show nothing to do
    if install_plan.as_ref().is_none_or(|p| p.is_empty())
        && update_plan.as_ref().is_none_or(|p| p.is_empty())
    {
        writeln!(writer)?;
        writeln!(writer, "Nothing to do.")?;
        return Ok(());
    }

    if dry_run {
        return Ok(());
    }

    writeln!(writer)?;
    if !crate::plan::prompt_confirm(reader, writer) {
        writeln!(writer, "Aborted.")?;
        return Ok(());
    }

    // Collect enabled packages from both plans
    let install_enabled: HashSet<&str> = install_plan
        .as_ref()
        .map(|p| p.enabled.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();
    let update_enabled: HashSet<&str> = update_plan
        .as_ref()
        .map(|p| p.enabled.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    let mut had_errors = false;

    // Execute in unified dependency order
    for (name, action) in &ordered_actions {
        let is_enabled = match action {
            Action::Install => install_enabled.contains(name.as_str()),
            Action::Update => update_enabled.contains(name.as_str()),
            Action::Uninstall => false,
        };
        if !is_enabled {
            continue;
        }

        let pkg_config = &config.packages[name];
        let script_name = resolve_script_name(pkg_config, *action);
        let script_path = ctx.packages_dir().join(name).join(&script_name);

        if !script_path.exists() {
            writeln!(writer, "Error: Script not found: {}", script_path.display())?;
            had_errors = true;
            continue;
        }

        let verb = action.gerund();
        writeln!(writer, "{verb} {name}...")?;

        match execute_script(&script_path) {
            Ok(_) => {
                writeln!(writer, "done")?;
                update_state_per_package(ctx, *action, name)?;
            }
            Err(e) => {
                writeln!(writer, "Error: {e}")?;
                writeln!(writer, "FAILED")?;
                had_errors = true;
            }
        }
    }

    if had_errors {
        writeln!(writer, "Some packages failed")?;
    }

    Ok(())
}

pub fn install(
    ctx: &Context,
    packages: &[String],
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_action(
        ctx,
        packages,
        Action::Install,
        dry_run,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub fn update(
    ctx: &Context,
    packages: &[String],
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_action(
        ctx,
        packages,
        Action::Update,
        dry_run,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub fn uninstall(
    ctx: &Context,
    packages: &[String],
    all: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    uninstall_to(
        ctx,
        packages,
        all,
        dry_run,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub(crate) fn uninstall_to<R: BufRead, W: Write>(
    ctx: &Context,
    packages: &[String],
    all: bool,
    dry_run: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved_packages = if all {
        let state_path = ctx.state_path();
        if state_path.exists() {
            State::load(&state_path)?.installed
        } else {
            Vec::new()
        }
    } else {
        packages.to_vec()
    };

    run_action(
        ctx,
        &resolved_packages,
        Action::Uninstall,
        dry_run,
        reader,
        writer,
    )
}

/// Execute an action for the given packages, with confirmation prompt.
/// I/O is injectable for testability. Reusable for install/update/uninstall.
pub fn run_action<R: BufRead, W: Write>(
    ctx: &Context,
    packages: &[String],
    action: Action,
    dry_run: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let state_path = ctx.state_path();
    let installed = if state_path.exists() {
        State::load(&state_path)?.installed
    } else {
        Vec::new()
    };

    // For install, expand to include transitive dependencies and sort topologically.
    // For uninstall, expand reverse deps (dependents) + forward deps, then reverse order.
    let mut plan_notes = std::collections::BTreeMap::new();
    let mut cycle_packages = Vec::new();
    let ordered_packages = match action {
        Action::Install => {
            let (expanded, notes) = expand_dependencies(&config, packages);
            plan_notes = notes;
            let topo_result = topological_sort(&config, &expanded)?;
            cycle_packages = topo_result.cycle;
            topo_result.sorted
        }
        Action::Uninstall => {
            // Expand reverse dependencies only — include packages that transitively
            // depend on the requested ones. Forward dependencies are intentionally NOT
            // included: removing them is outside the scope of what the user requested
            // and other packages may still rely on them.
            let (reverse_expanded, notes) = expand_reverse_dependencies(&config, packages);
            plan_notes = notes;
            let topo_result = topological_sort(&config, &reverse_expanded)?;
            cycle_packages = topo_result.cycle;
            let mut sorted = topo_result.sorted;
            sorted.reverse();
            sorted
        }
        Action::Update => packages.to_vec(),
    };

    let mut plan = Plan::build(
        &config,
        &ordered_packages,
        action,
        &installed,
        Some(&ctx.packages_dir()),
    )?;
    plan.notes = plan_notes;
    plan.circular_dependency = cycle_packages;

    if plan.is_empty() {
        let display = plan.display();
        if !display.is_empty() {
            writeln!(writer, "{display}")?;
            writeln!(writer)?;
        }
        writeln!(writer, "Nothing to do.")?;
        return Ok(());
    }

    if dry_run {
        let display = plan.display();
        writeln!(writer, "{display}")?;
        return Ok(());
    }

    if !confirm_plan(&plan, reader, writer) {
        writeln!(writer, "Aborted.")?;
        return Ok(());
    }

    let verb = action.gerund();

    let mut had_errors = false;

    for name in &plan.enabled {
        let pkg_config = &config.packages[name];
        let script_name = resolve_script_name(pkg_config, action);
        let script_path = ctx.packages_dir().join(name).join(&script_name);

        if !script_path.exists() {
            writeln!(writer, "Error: Script not found: {}", script_path.display())?;
            had_errors = true;
            continue;
        }

        writeln!(writer, "{verb} {name}...")?;

        match execute_script(&script_path) {
            Ok(_) => {
                writeln!(writer, "done")?;
                update_state_per_package(ctx, action, name)?;
            }
            Err(e) => {
                writeln!(writer, "Error: {e}")?;
                writeln!(writer, "FAILED")?;
                had_errors = true;
            }
        }
    }

    if had_errors {
        writeln!(writer, "Some packages failed")?;
    }

    Ok(())
}

/// Update state.yml for a single package after successful script execution.
fn update_state_per_package(
    ctx: &Context,
    action: Action,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let state_path = ctx.state_path();

    match action {
        Action::Install => {
            let mut state = if state_path.exists() {
                State::load(&state_path)?
            } else {
                State::default()
            };
            if !state.installed.contains(&name.to_string()) {
                state.installed.push(name.to_string());
            }
            state.save(&state_path)?;
        }
        Action::Uninstall => {
            if state_path.exists() {
                let mut state = State::load(&state_path)?;
                state.installed.retain(|n| n != name);
                state.save(&state_path)?;
            }

            let config_path = ctx.config_path();
            let mut config = Config::load(&config_path)?;
            if let Some(pkg) = config.packages.get_mut(name)
                && pkg.enabled
            {
                pkg.enabled = false;
                config.save(&config_path)?;
            }
        }
        Action::Update => {}
    }

    Ok(())
}

/// Resolve the script filename for a given action, considering aliases.
fn resolve_script_name(pkg_config: &PackageConfig, action: Action) -> String {
    let action_str = action.as_str();
    let resolved_action = pkg_config
        .script_aliases
        .get(action_str)
        .map(|s| s.as_str())
        .unwrap_or(action_str);
    let ext = super::script_extension();
    format!("{resolved_action}.{ext}")
}

/// Execute a script file via the OS-appropriate shell with inherited stdin/stdout/stderr.
fn execute_script(script_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new(super::shell_command())
        .arg(script_path)
        .status()?;
    if !status.success() {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(HomeosError::new(
            reasons::SCRIPT_FAILED,
            format!("Script failed with exit code {code}"),
        )
        .into());
    }
    Ok(())
}

/// Expand a list of packages to include all transitive dependencies.
/// Returns the expanded set as a Vec in no particular order, and a map of
/// pulled-in packages to "required by {requester}" notes for plan display.
/// The most direct requester is recorded (e.g., for A → B → C, C's requester is B).
fn expand_dependencies(
    config: &Config,
    packages: &[String],
) -> (Vec<String>, std::collections::BTreeMap<String, String>) {
    let requested: HashSet<String> = packages.iter().cloned().collect();
    let mut result: Vec<String> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut notes: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut stack: Vec<(String, Option<String>)> =
        packages.iter().map(|p| (p.clone(), None)).collect();

    while let Some((name, requester)) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(req) = requester
            && !requested.contains(&name)
        {
            notes.insert(name.clone(), format!("required by {req}"));
        }
        result.push(name.clone());
        if let Some(pkg_config) = config.packages.get(&name) {
            for dep in &pkg_config.depends_on {
                if !visited.contains(dep) {
                    stack.push((dep.clone(), Some(name.clone())));
                }
            }
        }
    }

    (result, notes)
}

/// Expand a list of packages to include all reverse (dependent) packages.
/// For each requested package, finds all packages that depend on it (recursively).
/// Returns the expanded list and a map of added packages to their dependency notes
/// (e.g., "depends on B" for packages pulled in because they depend on B).
fn expand_reverse_dependencies(
    config: &Config,
    packages: &[String],
) -> (Vec<String>, std::collections::BTreeMap<String, String>) {
    use std::collections::HashMap;

    // Build reverse dependency graph: dep -> packages that depend on it
    let mut reverse_graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for (name, pkg_config) in &config.packages {
        for dep in &pkg_config.depends_on {
            reverse_graph
                .entry(dep.as_str())
                .or_default()
                .push(name.as_str());
        }
    }

    let requested: HashSet<String> = packages.iter().cloned().collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut notes: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    // Stack: (package_name, the package it depends on that triggered inclusion)
    let mut stack: Vec<(String, Option<String>)> =
        packages.iter().map(|p| (p.clone(), None)).collect();

    while let Some((name, triggered_by)) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        // If this package was pulled in via reverse dep expansion, note it
        if let Some(dep_name) = triggered_by
            && !requested.contains(&name)
        {
            notes.insert(name.clone(), format!("depends on {dep_name}"));
        }
        // Find packages that depend on `name` — they also need to be uninstalled
        if let Some(dependents) = reverse_graph.get(name.as_str()) {
            for &dependent in dependents {
                if !visited.contains(dependent) {
                    stack.push((dependent.to_string(), Some(name.clone())));
                }
            }
        }
    }

    let result: Vec<String> = visited.into_iter().collect();
    (result, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::package::script_extension;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(data_dir));

        std::fs::create_dir_all(ctx.config_path().parent().unwrap()).unwrap();
        std::fs::write(ctx.config_path(), yaml).unwrap();

        (tmp, ctx)
    }

    fn fixture_with_script(
        yaml: &str,
        pkg: &str,
        action: &str,
        marker_path: &Path,
    ) -> (TempDir, Context) {
        let (tmp, ctx) = fixture(yaml);
        let pkg_dir = ctx.packages_dir().join(pkg);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let script_path = pkg_dir.join(format!("{action}.{ext}"));
        std::fs::write(
            &script_path,
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
        (tmp, ctx)
    }

    #[test]
    fn test_resolve_script_name_default() {
        // Arrange
        let pkg_config = PackageConfig::default();

        // Act
        let sut = resolve_script_name(&pkg_config, Action::Install);

        // Assert
        let ext = script_extension();
        assert_eq!(sut, format!("install.{ext}"));
    }

    #[test]
    fn test_resolve_script_name_with_alias() {
        // Arrange
        let pkg_config = PackageConfig {
            script_aliases: std::collections::BTreeMap::from([(
                "update".to_string(),
                "install".to_string(),
            )]),
            ..Default::default()
        };

        // Act
        let sut = resolve_script_name(&pkg_config, Action::Update);

        // Assert
        let ext = script_extension();
        assert_eq!(sut, format!("install.{ext}"));
    }

    #[test]
    fn test_execute_script_creates_side_effect() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let marker_path = tmp.path().join("marker.txt");
        let ext = script_extension();
        let script_path = tmp.path().join(format!("test.{ext}"));
        let content = if cfg!(windows) {
            format!(
                "New-Item -Path '{}' -ItemType File | Out-Null\n",
                marker_path.display()
            )
        } else {
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display())
        };
        std::fs::write(&script_path, content).unwrap();

        // Act
        execute_script(&script_path).unwrap();

        // Assert
        assert!(marker_path.exists());
    }

    #[test]
    fn test_execute_script_returns_error_on_failure() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let ext = script_extension();
        let script_path = tmp.path().join(format!("fail.{ext}"));
        let content = if cfg!(windows) {
            "exit 1\n".to_string()
        } else {
            "#!/usr/bin/env sh\nexit 1\n".to_string()
        };
        std::fs::write(&script_path, content).unwrap();

        // Act
        let result = execute_script(&script_path);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exit code 1"));
    }

    #[test]
    fn test_run_action_executes_install_scripts() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(marker_path.exists());
    }

    #[test]
    fn test_run_action_skips_disabled_packages() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Nothing to do."));
        assert!(!written.contains("Installing"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_run_action_aborts_on_no_confirmation() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Installing"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_run_action_reports_missing_script_and_continues() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir().join("neovim")).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Script not found"));
        assert!(written.contains("Some packages failed"));
    }

    #[test]
    fn test_run_action_respects_script_aliases() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("alias_marker");
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    script_aliases:\n      update: install\n");
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        // Only create install script (update is overridden to install)
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(marker_path.exists());
    }

    #[test]
    fn test_run_action_executes_update_scripts() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("update_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(marker_path.exists());
    }

    #[test]
    fn test_run_action_skips_disabled_packages_for_update() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "update",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Nothing to do."));
        assert!(!written.contains("Updating"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_run_action_aborts_update_on_no_confirmation() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Updating"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_run_action_executes_uninstall_scripts() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
        assert!(marker_path.exists());
    }

    #[test]
    fn test_run_action_executes_disabled_packages_for_uninstall() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(!written.contains("neovim (disabled)"));
        assert!(written.contains("Uninstalling neovim"));
        assert!(marker_path.exists());
    }

    #[test]
    fn test_run_action_aborts_uninstall_on_no_confirmation() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        State {
            installed: vec!["neovim".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Uninstalling"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_run_action_executes_multiple_packages() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(written.contains("Installing ripgrep...\ndone"));
        assert!(marker_dir.path().join("neovim_marker").exists());
        assert!(marker_dir.path().join("ripgrep_marker").exists());
    }

    #[test]
    fn test_install_records_package_in_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
    }

    #[test]
    fn test_install_creates_state_file_if_missing() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        assert!(!ctx.state_path().exists());
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        assert!(ctx.state_path().exists());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
    }

    #[test]
    fn test_install_appends_to_existing_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  ripgrep: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        // Pre-populate state with ripgrep
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep", "neovim"]);
    }

    #[test]
    fn test_install_does_not_duplicate_in_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        // Pre-populate state with neovim already installed
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
    }

    #[test]
    fn test_uninstall_removes_package_from_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
    }

    #[test]
    fn test_uninstall_noop_when_no_state_file() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        assert!(!ctx.state_path().exists());
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        assert!(!ctx.state_path().exists());
    }

    #[test]
    fn test_uninstall_removes_multiple_packages_from_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec![
                "neovim".to_string(),
                "ripgrep".to_string(),
                "zed".to_string(),
            ],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["zed"]);
    }

    #[test]
    fn test_uninstall_ignores_package_not_in_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
    }

    #[test]
    fn test_update_does_not_record_in_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("update_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        assert!(!ctx.state_path().exists());
    }

    #[test]
    fn test_install_skips_already_installed_package() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (already installed)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Nothing to do."));
        assert!(!written.contains("Installing"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_install_skips_already_installed_but_installs_new() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let zed_marker = marker_dir.path().join("zed_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  zed: {}\n",
            "neovim",
            "install",
            &neovim_marker,
        );
        // Also create install script for zed
        let zed_dir = ctx.packages_dir().join("zed");
        std::fs::create_dir_all(&zed_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            zed_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", zed_marker.display()),
        )
        .unwrap();
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string(), "zed".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (already installed)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Installing zed...\ndone"));
        assert!(!written.contains("Installing neovim"));
        assert!(!neovim_marker.exists());
        assert!(zed_marker.exists());
    }

    #[test]
    fn test_install_all_already_installed_shows_no_packages() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  zed: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string(), "zed".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string(), "zed".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (already installed)"));
        assert!(written.contains("zed (already installed)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Nothing to do."));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_install_records_state_per_package_on_partial_failure() {
        // Arrange: neovim has a valid script, ripgrep has no script (will fail)
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();
        let ripgrep_dir = ctx.packages_dir().join("ripgrep");
        std::fs::create_dir_all(&ripgrep_dir).unwrap();
        // No install script for ripgrep
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(written.contains("Script not found"));
        assert!(written.contains("Some packages failed"));
        assert!(neovim_marker.exists());
    }

    #[test]
    fn test_install_continues_after_script_failure() {
        // Arrange: neovim has a failing script, ripgrep has a valid script
        let marker_dir = TempDir::new().unwrap();
        let ripgrep_marker = marker_dir.path().join("ripgrep_marker");
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\nexit 1\n",
        )
        .unwrap();
        let ripgrep_dir = ctx.packages_dir().join("ripgrep");
        std::fs::create_dir_all(&ripgrep_dir).unwrap();
        std::fs::write(
            ripgrep_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", ripgrep_marker.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains("Installing neovim...\nError: Script failed with exit code 1\nFAILED")
        );
        assert!(written.contains("Installing ripgrep...\ndone"));
        assert!(written.contains("Some packages failed"));
        assert!(ripgrep_marker.exists());
    }

    #[test]
    fn test_uninstall_records_state_per_package() {
        // Arrange: two packages with valid uninstall scripts
        let marker_dir = TempDir::new().unwrap();
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ext = script_extension();
        for pkg in &["neovim", "ripgrep"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(state.installed.is_empty());
    }

    #[test]
    fn test_uninstall_records_state_per_package_on_partial_failure() {
        // Arrange: neovim has a valid script, ripgrep has no script
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("uninstall.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();
        let ripgrep_dir = ctx.packages_dir().join("ripgrep");
        std::fs::create_dir_all(&ripgrep_dir).unwrap();
        // No uninstall script for ripgrep
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
        assert!(written.contains("Script not found"));
        assert!(written.contains("Some packages failed"));
        assert!(neovim_marker.exists());
    }

    #[test]
    fn test_uninstall_disables_package_in_config() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
    }

    #[test]
    fn test_uninstall_disables_multiple_packages_in_config() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
        assert!(!config.packages["ripgrep"].enabled);
    }

    #[test]
    fn test_uninstall_does_not_disable_on_failure() {
        // Arrange: neovim has no script, so uninstall will fail
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        std::fs::create_dir_all(ctx.packages_dir().join("neovim")).unwrap();
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let _ = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert: package should still be enabled since uninstall failed
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages["neovim"].enabled);
    }

    #[test]
    fn test_uninstall_already_disabled_stays_disabled() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — disabled packages are skipped by the plan, so uninstall is a no-op
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert: package remains disabled
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.packages["neovim"].enabled);
    }

    #[test]
    fn test_update_does_not_skip_already_installed() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("update_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(!written.contains("already installed"));
    }

    #[test]
    fn test_uninstall_all_uninstalls_all_installed_packages() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
        assert!(written.contains("Uninstalling ripgrep...\ndone"));
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(state.installed.is_empty());
    }

    #[test]
    fn test_uninstall_all_with_no_state_file() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_uninstall_all_with_empty_state() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let state = State { installed: vec![] };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_uninstall_all_shows_confirmation_prompt() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("will be uninstalled"));
        assert!(written.contains("neovim"));
        assert!(written.contains("Proceed? [y/N]"));
        assert!(written.contains("Aborted."));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_update_loads_state_for_plan() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("update_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        );

        // Assert — state is loaded but update still executes in-state packages
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
    }

    #[test]
    fn test_uninstall_loads_state_for_plan() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        );

        // Assert — state is loaded but uninstall still executes in-state packages
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
    }

    #[test]
    fn test_uninstall_all_ignores_packages_arg() {
        // Arrange: --all flag set, packages arg is empty, state has packages
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("uninstall_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
    }

    // --- expand_dependencies tests ---

    #[test]
    fn test_expand_dependencies_no_deps() {
        // Arrange
        let config = Config {
            packages: std::collections::BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig::default(),
            )]),
            ..Default::default()
        };

        // Act
        let (sut, notes) = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert
        assert_eq!(sut, vec!["neovim"]);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_expand_dependencies_includes_transitive() {
        // Arrange — neovim depends on git, git depends on curl
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "neovim".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                (
                    "git".to_string(),
                    PackageConfig {
                        depends_on: vec!["curl".to_string()],
                        ..Default::default()
                    },
                ),
                ("curl".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (sut, _notes) = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert — all three packages included
        let mut sorted = sut.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["curl", "git", "neovim"]);
    }

    #[test]
    fn test_expand_dependencies_no_duplicates() {
        // Arrange — both neovim and zed depend on git
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "neovim".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                (
                    "zed".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                ("git".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (sut, _notes) =
            expand_dependencies(&config, &["neovim".to_string(), "zed".to_string()]);

        // Assert — git appears only once
        let git_count = sut.iter().filter(|s| s.as_str() == "git").count();
        assert_eq!(git_count, 1);
        assert_eq!(sut.len(), 3);
    }

    #[test]
    fn test_expand_dependencies_unknown_dep_included_as_is() {
        // Arrange — neovim depends on unknown_pkg which is not in config
        let config = Config {
            packages: std::collections::BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig {
                    depends_on: vec!["unknown_pkg".to_string()],
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let (sut, _notes) = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert — unknown_pkg is included (Plan::build will error on it)
        let mut sorted = sut.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["neovim", "unknown_pkg"]);
    }

    #[test]
    fn test_expand_dependencies_annotates_direct_dependency() {
        // Arrange — neovim depends on git; only request neovim
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "neovim".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                ("git".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (_expanded, notes) = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert
        assert_eq!(notes.get("git").unwrap(), "required by neovim");
        assert!(!notes.contains_key("neovim")); // requested package has no note
    }

    #[test]
    fn test_expand_dependencies_annotates_transitive_with_most_direct_requester() {
        // Arrange — a depends on b, b depends on c; only request a
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "a".to_string(),
                    PackageConfig {
                        depends_on: vec!["b".to_string()],
                        ..Default::default()
                    },
                ),
                (
                    "b".to_string(),
                    PackageConfig {
                        depends_on: vec!["c".to_string()],
                        ..Default::default()
                    },
                ),
                ("c".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (_expanded, notes) = expand_dependencies(&config, &["a".to_string()]);

        // Assert — c is required by b (its direct requester), not a
        assert_eq!(notes.get("b").unwrap(), "required by a");
        assert_eq!(notes.get("c").unwrap(), "required by b");
        assert!(!notes.contains_key("a"));
    }

    #[test]
    fn test_expand_dependencies_no_note_for_explicitly_requested_package() {
        // Arrange — a depends on b; both requested explicitly
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "a".to_string(),
                    PackageConfig {
                        depends_on: vec!["b".to_string()],
                        ..Default::default()
                    },
                ),
                ("b".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (_expanded, notes) = expand_dependencies(&config, &["a".to_string(), "b".to_string()]);

        // Assert — neither gets a note since both were explicitly requested
        assert!(notes.is_empty());
    }

    // --- Dependency ordering integration tests ---

    #[test]
    fn test_install_includes_dependencies_in_order() {
        // Arrange — neovim depends on git; only request neovim
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        // Create install scripts for both packages
        let ext = script_extension();
        for pkg in ["neovim", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }

        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — only request neovim, git should be pulled in as dependency
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — git installed before neovim
        let written = String::from_utf8(output).unwrap();
        let git_pos = written
            .find("Installing git")
            .expect("git should be installed");
        let neovim_pos = written
            .find("Installing neovim")
            .expect("neovim should be installed");
        assert!(git_pos < neovim_pos, "git must be installed before neovim");
        assert!(marker_dir.path().join("git_marker").exists());
        assert!(marker_dir.path().join("neovim_marker").exists());
    }

    #[test]
    fn test_install_plan_annotates_pulled_in_dependencies() {
        // Arrange — neovim depends on git, git depends on curl; only request neovim
        let marker_dir = TempDir::new().unwrap();
        let yaml = concat!(
            "packages:\n",
            "  neovim:\n",
            "    depends_on:\n",
            "      - git\n",
            "  git:\n",
            "    depends_on:\n",
            "      - curl\n",
            "  curl: {}\n",
        );
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        for pkg in ["neovim", "git", "curl"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }

        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act — only request neovim
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — plan shows pulled-in deps annotated with their direct requester,
        // neovim (explicitly requested) has no annotation
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("git (required by neovim)"));
        assert!(written.contains("curl (required by git)"));
        assert!(written.contains("  neovim\n"));
    }

    #[test]
    fn test_install_dependencies_recorded_in_state() {
        // Arrange — neovim depends on git; only request neovim
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        for pkg in ["neovim", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }

        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — both packages recorded in state
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(state.installed.contains(&"git".to_string()));
        assert!(state.installed.contains(&"neovim".to_string()));
    }

    #[test]
    fn test_install_skips_already_installed_dependency() {
        // Arrange — neovim depends on git; git already installed
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();

        // git is already installed
        let state = State {
            installed: vec!["git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();

        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — git skipped as already installed, neovim installed
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("git (already installed)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(neovim_marker.exists());
    }

    #[test]
    fn test_install_circular_dependency_skips_gracefully() {
        // Arrange — a depends on b, b depends on a
        let yaml =
            "packages:\n  a:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["a".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — both packages skipped with circular dependency, nothing to do
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_uninstall_does_not_pull_in_forward_dependencies() {
        // Arrange — neovim depends on git. Uninstalling neovim must NOT also uninstall
        // git: git is a forward dep that the user did not request to remove and may
        // still be needed by other packages on the machine.
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["neovim", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["git".to_string(), "neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — only request neovim; git must stay
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — neovim uninstalled, git never appears in the plan and stays in state
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
        assert!(
            !written.contains("Uninstalling git"),
            "git must not be uninstalled (forward dep); output:\n{written}"
        );
        assert!(
            !written.contains("git"),
            "git must not appear in plan at all; output:\n{written}"
        );
        assert!(marker_dir.path().join("neovim_marker").exists());
        assert!(!marker_dir.path().join("git_marker").exists());
        let state_after = State::load(&ctx.state_path()).unwrap();
        assert!(state_after.installed.contains(&"git".to_string()));
        assert!(!state_after.installed.contains(&"neovim".to_string()));
    }

    #[test]
    fn test_uninstall_chain_dependency_does_not_pull_in_forward_deps() {
        // Arrange — c depends on b, b depends on a; only c is requested for uninstall.
        // After the forward-dep removal: only c is uninstalled. a and b stay.
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  c:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n  a: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["a", "b", "c"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["c".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — only c uninstalled; a and b remain in state and have no plan entries
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling c...\ndone"));
        assert!(!written.contains("Uninstalling b"));
        assert!(!written.contains("Uninstalling a"));
        let state_after = State::load(&ctx.state_path()).unwrap();
        assert!(state_after.installed.contains(&"a".to_string()));
        assert!(state_after.installed.contains(&"b".to_string()));
        assert!(!state_after.installed.contains(&"c".to_string()));
    }

    #[test]
    fn test_uninstall_does_not_classify_forward_dep_as_not_installed() {
        // Arrange — neovim depends on git; git is not in state. The forward dep should
        // not appear in the plan at all (not even as `(not installed)`), because
        // forward-dep expansion was removed for uninstall.
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("uninstall.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — neovim uninstalled, git not in any section of the plan
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim...\ndone"));
        assert!(
            !written.contains("git"),
            "git must not appear in plan; output:\n{written}"
        );
        assert!(neovim_marker.exists());
    }

    #[test]
    fn test_uninstall_circular_dependency_skips_gracefully() {
        // Arrange — a depends on b, b depends on a, both installed
        let yaml =
            "packages:\n  a:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n";
        let (_tmp, ctx) = fixture(yaml);
        let state = State {
            installed: vec!["a".to_string(), "b".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["a".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — both packages skipped with circular dependency, nothing to do
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_uninstall_does_not_remove_forward_dep_from_state() {
        // Arrange — neovim depends on git; both installed
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["neovim", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["git".to_string(), "neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — neovim removed, git remains (git is a forward dep, untouched)
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(!state.installed.contains(&"neovim".to_string()));
        assert!(state.installed.contains(&"git".to_string()));
    }

    #[test]
    fn test_update_does_not_expand_dependencies() {
        // Arrange — neovim depends on git; only request neovim for update
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_marker");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("update.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();

        // neovim is installed (required for update)
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();

        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — only neovim updated, git not mentioned
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(!written.contains("git"));
        assert!(neovim_marker.exists());
    }

    // --- apply_to tests ---

    fn write_script(ctx: &Context, pkg: &str, action: &str, marker_path: &Path) {
        let pkg_dir = ctx.packages_dir().join(pkg);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let script_path = pkg_dir.join(format!("{action}.{ext}"));
        std::fs::write(
            &script_path,
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
    }

    #[test]
    fn test_apply_installs_enabled_not_in_state_and_updates_enabled_in_state() {
        // Arrange: neovim is enabled+not-in-state (install), zed is enabled+in-state (update)
        let marker_dir = TempDir::new().unwrap();
        let neo_marker = marker_dir.path().join("neo_install");
        let zed_marker = marker_dir.path().join("zed_update");
        let yaml = "packages:\n  neovim: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", &neo_marker);
        write_script(&ctx, "zed", "update", &zed_marker);
        let state = State {
            installed: vec!["zed".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(written.contains("Updating zed...\ndone"));
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(state.installed.contains(&"neovim".to_string()));
        assert!(state.installed.contains(&"zed".to_string()));
        assert!(neo_marker.exists());
        assert!(zed_marker.exists());
    }

    #[test]
    fn test_apply_skips_disabled_packages() {
        // Arrange: neovim is disabled, zed is enabled+not-in-state
        let marker_dir = TempDir::new().unwrap();
        let zed_marker = marker_dir.path().join("zed_install");
        let yaml = "packages:\n  neovim:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", &zed_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing zed...\ndone"));
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("will be skipped"));
        assert!(!written.contains("Installing neovim"));
        assert!(zed_marker.exists());
    }

    #[test]
    fn test_apply_nothing_to_do_when_all_disabled() {
        // Arrange: all packages disabled
        let yaml = "packages:\n  neovim:\n    enabled: false\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_apply_nothing_to_do_when_no_packages() {
        // Arrange: empty config
        let yaml = "packages: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_apply_aborts_on_no_confirmation() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", &marker_path);
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Installing"));
        assert!(!marker_path.exists());
    }

    #[test]
    fn test_apply_only_installs_when_nothing_in_state() {
        // Arrange: two enabled packages, no state
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_install");
        let neo_marker = marker_dir.path().join("neo_install");
        let yaml = "packages:\n  git: {}\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "install", &git_marker);
        write_script(&ctx, "neovim", "install", &neo_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing git...\ndone"));
        assert!(written.contains("Installing neovim...\ndone"));
        assert!(!written.contains("Updating"));
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed.len(), 2);
    }

    #[test]
    fn test_apply_only_updates_when_all_in_state() {
        // Arrange: two enabled packages, both in state
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_update");
        let neo_marker = marker_dir.path().join("neo_update");
        let yaml = "packages:\n  git: {}\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", &git_marker);
        write_script(&ctx, "neovim", "update", &neo_marker);
        let state = State {
            installed: vec!["git".to_string(), "neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating git...\ndone"));
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(!written.contains("Installing"));
    }

    #[test]
    fn test_apply_records_installed_in_state() {
        // Arrange: neovim enabled+not-in-state
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_neo");
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", &marker_path);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
    }

    #[test]
    fn test_apply_shows_combined_plan() {
        // Arrange: neovim to install, zed to update
        let marker_dir = TempDir::new().unwrap();
        let neo_marker = marker_dir.path().join("neo_install");
        let zed_marker = marker_dir.path().join("zed_update");
        let yaml = "packages:\n  neovim: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", &neo_marker);
        write_script(&ctx, "zed", "update", &zed_marker);
        let state = State {
            installed: vec!["zed".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("will be installed"));
        assert!(written.contains("will be updated"));
        assert!(written.contains("Proceed? [y/N]"));
    }

    #[test]
    fn test_apply_updates_dependency_before_installing_dependent() {
        // Arrange: neovim depends on git. git is already installed (update), neovim is new (install).
        // git must be updated before neovim is installed.
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_update");
        let neo_marker = marker_dir.path().join("neo_install");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", &git_marker);
        write_script(&ctx, "neovim", "install", &neo_marker);
        let state = State {
            installed: vec!["git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: git updated before neovim installed
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Updating git...\ndone").unwrap();
        let neo_pos = written.find("Installing neovim...\ndone").unwrap();
        assert!(
            git_pos < neo_pos,
            "git should be updated before neovim is installed"
        );
    }

    #[test]
    fn test_apply_annotates_pulled_in_update_with_required_by() {
        // Arrange: neovim (new install) depends on git (already installed → update).
        // apply expands [neovim]'s deps and pulls git in. git ends up in the update plan
        // but should be annotated with "required by neovim" since it was pulled in via
        // forward expansion from neovim.
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_update");
        let neo_marker = marker_dir.path().join("neo_install");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", &git_marker);
        write_script(&ctx, "neovim", "install", &neo_marker);
        let state = State {
            installed: vec!["git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: git is annotated under the update section; neovim has no annotation
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("git (required by neovim)"));
        assert!(written.contains("  neovim\n"));
    }

    #[test]
    fn test_apply_annotates_intra_set_direct_dependency() {
        // Arrange: both neovim and git are enabled+not-in-state. neovim depends on git.
        // Both come from the enabled set (implicitly requested), so the dep relationship
        // within the install set should be annotated — git is required by neovim.
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_install");
        let neo_marker = marker_dir.path().join("neo_install");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "install", &git_marker);
        write_script(&ctx, "neovim", "install", &neo_marker);
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: git is annotated as required by neovim; neovim has no annotation
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains("git (required by neovim)"),
            "expected 'git (required by neovim)' in output, got:\n{written}"
        );
        assert!(
            written.contains("  neovim\n"),
            "expected unannotated '  neovim' in output, got:\n{written}"
        );
    }

    #[test]
    fn test_apply_annotates_intra_set_transitive_dependencies() {
        // Arrange: chain a → b → c (a depends on b, b depends on c). All are
        // enabled+not-in-state. Each pulled-in dep should be annotated with its
        // most direct (immediate parent) requester: c is required by b, b by a,
        // a by nobody.
        let marker_dir = TempDir::new().unwrap();
        let a_marker = marker_dir.path().join("a_install");
        let b_marker = marker_dir.path().join("b_install");
        let c_marker = marker_dir.path().join("c_install");
        let yaml = "packages:\n  a:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - c\n  c: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "a", "install", &a_marker);
        write_script(&ctx, "b", "install", &b_marker);
        write_script(&ctx, "c", "install", &c_marker);
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains("c (required by b)"),
            "expected 'c (required by b)' in output, got:\n{written}"
        );
        assert!(
            written.contains("b (required by a)"),
            "expected 'b (required by a)' in output, got:\n{written}"
        );
        assert!(
            written.contains("  a\n"),
            "expected unannotated '  a' in output, got:\n{written}"
        );
    }

    #[test]
    fn test_apply_intra_set_picks_alphabetically_first_requester() {
        // Arrange: both alpha and beta directly depend on shared. All enabled+not-in-state.
        // shared has two direct requesters in the set; the alphabetically first ('alpha')
        // should be recorded as the requester.
        let marker_dir = TempDir::new().unwrap();
        let a_marker = marker_dir.path().join("alpha_install");
        let b_marker = marker_dir.path().join("beta_install");
        let s_marker = marker_dir.path().join("shared_install");
        let yaml = "packages:\n  alpha:\n    depends_on:\n      - shared\n  beta:\n    depends_on:\n      - shared\n  shared: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "alpha", "install", &a_marker);
        write_script(&ctx, "beta", "install", &b_marker);
        write_script(&ctx, "shared", "install", &s_marker);
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains("shared (required by alpha)"),
            "expected 'shared (required by alpha)' (deterministic alphabetical choice), got:\n{written}"
        );
    }

    #[test]
    fn test_apply_topological_order_for_install_chain() {
        // Arrange: c depends on b, b depends on a. All are new installs.
        let marker_dir = TempDir::new().unwrap();
        let a_marker = marker_dir.path().join("a_install");
        let b_marker = marker_dir.path().join("b_install");
        let c_marker = marker_dir.path().join("c_install");
        let yaml = "packages:\n  c:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n  a: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "a", "install", &a_marker);
        write_script(&ctx, "b", "install", &b_marker);
        write_script(&ctx, "c", "install", &c_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: a before b before c
        let written = String::from_utf8(output).unwrap();
        let a_pos = written.find("Installing a...\ndone").unwrap();
        let b_pos = written.find("Installing b...\ndone").unwrap();
        let c_pos = written.find("Installing c...\ndone").unwrap();
        assert!(a_pos < b_pos, "a should be installed before b");
        assert!(b_pos < c_pos, "b should be installed before c");
    }

    #[test]
    fn test_apply_topological_order_for_updates() {
        // Arrange: neovim depends on git. Both are already installed (both update).
        // git should be updated before neovim.
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_update");
        let neo_marker = marker_dir.path().join("neo_update");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", &git_marker);
        write_script(&ctx, "neovim", "update", &neo_marker);
        let state = State {
            installed: vec!["git".to_string(), "neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: git updated before neovim
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Updating git...\ndone").unwrap();
        let neo_pos = written.find("Updating neovim...\ndone").unwrap();
        assert!(git_pos < neo_pos, "git should be updated before neovim");
    }

    #[test]
    fn test_apply_expands_transitive_deps_for_install() {
        // Arrange: neovim depends on git (not in config as enabled but is a dep).
        // git is not in state — should be pulled in as an install dependency.
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_install");
        let neo_marker = marker_dir.path().join("neo_install");
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "install", &git_marker);
        write_script(&ctx, "neovim", "install", &neo_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: both installed, git first
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Installing git...\ndone").unwrap();
        let neo_pos = written.find("Installing neovim...\ndone").unwrap();
        assert!(git_pos < neo_pos, "git should be installed before neovim");
    }

    #[test]
    fn test_apply_mixed_install_update_diamond_dependency() {
        // Arrange: d depends on b and c, b and c depend on a.
        // a and b are already installed (update), c and d are new (install).
        let marker_dir = TempDir::new().unwrap();
        let a_marker = marker_dir.path().join("a_update");
        let b_marker = marker_dir.path().join("b_update");
        let c_marker = marker_dir.path().join("c_install");
        let d_marker = marker_dir.path().join("d_install");
        let yaml = "packages:\n  d:\n    depends_on:\n      - b\n      - c\n  c:\n    depends_on:\n      - a\n  b:\n    depends_on:\n      - a\n  a: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "a", "update", &a_marker);
        write_script(&ctx, "b", "update", &b_marker);
        write_script(&ctx, "c", "install", &c_marker);
        write_script(&ctx, "d", "install", &d_marker);
        let state = State {
            installed: vec!["a".to_string(), "b".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert: a before b and c, b and c before d
        let written = String::from_utf8(output).unwrap();
        let a_pos = written.find("Updating a...\ndone").unwrap();
        let b_pos = written.find("Updating b...\ndone").unwrap();
        let c_pos = written.find("Installing c...\ndone").unwrap();
        let d_pos = written.find("Installing d...\ndone").unwrap();
        assert!(a_pos < c_pos, "a should be updated before c is installed");
        assert!(a_pos < b_pos, "a should be updated before b is updated");
        assert!(b_pos < d_pos, "b should be updated before d is installed");
        assert!(c_pos < d_pos, "c should be installed before d is installed");
    }

    #[test]
    fn test_apply_shows_disabled_in_plan_with_enabled_packages() {
        // Arrange: neovim disabled, zed enabled+not-in-state, ripgrep enabled+in-state
        let marker_dir = TempDir::new().unwrap();
        let zed_marker = marker_dir.path().join("zed_install");
        let rg_marker = marker_dir.path().join("rg_update");
        let yaml = "packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", &zed_marker);
        write_script(&ctx, "ripgrep", "update", &rg_marker);
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("will be skipped"));
        assert!(written.contains("Installing zed...\ndone"));
        assert!(written.contains("Updating ripgrep...\ndone"));
    }

    #[test]
    fn test_apply_shows_multiple_disabled_packages() {
        // Arrange: two disabled packages, one enabled
        let marker_dir = TempDir::new().unwrap();
        let zed_marker = marker_dir.path().join("zed_install");
        let yaml =
            "packages:\n  docker:\n    enabled: false\n  neovim:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", &zed_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("docker (disabled)"));
        assert!(written.contains("neovim (disabled)"));
        assert!(written.contains("Installing zed...\ndone"));
    }

    #[test]
    fn test_run_action_skips_unmodified_skeleton_script() {
        // Arrange — install script still contains the "Generated by homeos" marker.
        // The package must be classified as skipped (script unmodified) and the script
        // must NOT execute. State.yml must NOT record neovim as installed.
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let marker_path = ctx.packages_dir().parent().unwrap().join("ran_install");
        // Skeleton with the marker AND a side-effect that would fire if executed.
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!(
                "#!/usr/bin/env sh\n# Generated by homeos — fill in install logic.\ntouch '{}'\n",
                marker_path.display()
            ),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains(&format!("neovim (script unmodified: install.{ext})")),
            "plan should show script unmodified skipped entry, got: {written}"
        );
        assert!(
            !written.contains("Installing neovim..."),
            "execution must not start for unmodified script, got: {written}"
        );
        assert!(
            !marker_path.exists(),
            "install script must not run when classified as script unmodified"
        );
        assert!(
            !ctx.state_path().exists(),
            "state.yml must not be created for skipped packages"
        );
    }

    #[test]
    fn test_run_action_executes_modified_script() {
        // Arrange — modified install script should execute and be recorded in state.yml.
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("install_marker");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let _ = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            !written.contains("script unmodified"),
            "should not mark modified script as unmodified"
        );
        assert!(
            written.contains("Installing neovim...\ndone"),
            "modified script should execute"
        );
        assert!(marker_path.exists(), "modified script should run");
    }

    #[test]
    fn test_apply_skips_unmodified_skeleton_script() {
        // Arrange
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let marker_path = ctx.packages_dir().parent().unwrap().join("ran_install");
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!(
                "#!/usr/bin/env sh\n# Generated by homeos — fill in install logic.\ntouch '{}'\n",
                marker_path.display()
            ),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains(&format!("neovim (script unmodified: install.{ext})")),
            "apply plan should show script unmodified skipped entry, got: {written}"
        );
        assert!(
            !marker_path.exists(),
            "install script must not run during apply when classified as script unmodified"
        );
        assert!(
            !ctx.state_path().exists(),
            "state.yml must not be created for skipped packages"
        );
    }

    #[test]
    fn test_apply_skips_unmodified_update_script_for_in_state_package() {
        // Arrange: neovim is enabled+in_state, so apply routes it to update.
        // The update script still contains the skeleton marker — apply must surface
        // it under the consolidated skipped section (not silently drop it), and
        // execution must not occur.
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let marker_path = ctx.packages_dir().parent().unwrap().join("ran_update");
        std::fs::write(
            pkg_dir.join(format!("update.{ext}")),
            format!(
                "#!/usr/bin/env sh\n# Generated by homeos — fill in update logic.\ntouch '{}'\n",
                marker_path.display()
            ),
        )
        .unwrap();
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains(&format!("neovim (script unmodified: update.{ext})")),
            "apply plan should show update-side script unmodified skipped entry, got: {written}"
        );
        assert!(
            !written.contains("Updating neovim..."),
            "execution must not start for unmodified update script, got: {written}"
        );
        assert!(
            !marker_path.exists(),
            "update script must not run during apply when classified as script unmodified"
        );
        assert_eq!(
            written
                .matches("The following packages will be skipped:")
                .count(),
            1,
            "exactly one skipped header expected, got: {written}"
        );
    }

    #[test]
    fn test_apply_consolidates_install_and_update_side_unmodified_entries() {
        // Arrange: neovim is enabled+not_in_state (install path) with unmodified
        // install script; zed is enabled+in_state (update path) with unmodified
        // update script. Both must appear under a SINGLE consolidated skipped
        // header in apply's plan display.
        let yaml = "packages:\n  neovim: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        let neo_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neo_dir).unwrap();
        std::fs::write(
            neo_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\n# Generated by homeos — fill in install logic.\n",
        )
        .unwrap();
        let zed_dir = ctx.packages_dir().join("zed");
        std::fs::create_dir_all(&zed_dir).unwrap();
        std::fs::write(
            zed_dir.join(format!("update.{ext}")),
            "#!/usr/bin/env sh\n# Generated by homeos — fill in update logic.\n",
        )
        .unwrap();
        let state = State {
            installed: vec!["zed".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains(&format!("neovim (script unmodified: install.{ext})")),
            "install-side unmodified entry expected, got: {written}"
        );
        assert!(
            written.contains(&format!("zed (script unmodified: update.{ext})")),
            "update-side unmodified entry expected, got: {written}"
        );
        assert_eq!(
            written
                .matches("The following packages will be skipped:")
                .count(),
            1,
            "exactly one skipped header expected, got: {written}"
        );
        assert!(
            !written.contains("Installing neovim..."),
            "install script must not execute, got: {written}"
        );
        assert!(
            !written.contains("Updating zed..."),
            "update script must not execute, got: {written}"
        );
    }

    #[test]
    fn test_apply_disabled_shown_before_prompt() {
        // Arrange: disabled package should appear in plan before confirmation prompt
        let marker_dir = TempDir::new().unwrap();
        let zed_marker = marker_dir.path().join("zed_install");
        let yaml = "packages:\n  docker:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", &zed_marker);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        let skip_pos = written.find("docker (disabled)").unwrap();
        let prompt_pos = written.find("Proceed? [y/N]").unwrap();
        assert!(
            skip_pos < prompt_pos,
            "disabled message should appear before confirmation prompt"
        );
    }

    #[test]
    fn test_script_failure_shows_error_before_failed() {
        // Arrange: a package with a failing script
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let ext = script_extension();
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\nexit 1\n",
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let _ = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        );

        // Assert: Error details appear before FAILED
        let written = String::from_utf8(output).unwrap();
        let error_pos = written.find("Error: Script failed").unwrap();
        let failed_pos = written.find("FAILED").unwrap();
        assert!(
            error_pos < failed_pos,
            "Error details should appear before FAILED"
        );
    }

    #[test]
    fn test_apply_script_failure_shows_error_before_failed() {
        // Arrange: a package with a failing install script
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n");
        let ext = script_extension();
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\nexit 1\n",
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let _ = apply_to(&ctx, false, &mut input, &mut output);

        // Assert: Error details appear before FAILED
        let written = String::from_utf8(output).unwrap();
        let error_pos = written.find("Error: Script failed").unwrap();
        let failed_pos = written.find("FAILED").unwrap();
        assert!(
            error_pos < failed_pos,
            "Error details should appear before FAILED"
        );
    }

    // --- expand_reverse_dependencies tests ---

    #[test]
    fn test_expand_reverse_dependencies_no_dependents() {
        // Arrange — neovim has no dependents
        let config = Config {
            packages: std::collections::BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig::default(),
            )]),
            ..Default::default()
        };

        // Act
        let (expanded, notes) = expand_reverse_dependencies(&config, &["neovim".to_string()]);

        // Assert
        assert_eq!(expanded, vec!["neovim"]);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_expand_reverse_dependencies_includes_direct_dependent() {
        // Arrange — editor depends on git; uninstalling git should include editor
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "editor".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                ("git".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (expanded, notes) = expand_reverse_dependencies(&config, &["git".to_string()]);

        // Assert
        let mut sorted = expanded.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["editor", "git"]);
        assert_eq!(notes.get("editor").unwrap(), "depends on git");
        assert!(!notes.contains_key("git")); // requested package has no note
    }

    #[test]
    fn test_expand_reverse_dependencies_transitive() {
        // Arrange — c depends on b, b depends on a; uninstalling a should include b and c
        let config = Config {
            packages: std::collections::BTreeMap::from([
                ("a".to_string(), PackageConfig::default()),
                (
                    "b".to_string(),
                    PackageConfig {
                        depends_on: vec!["a".to_string()],
                        ..Default::default()
                    },
                ),
                (
                    "c".to_string(),
                    PackageConfig {
                        depends_on: vec!["b".to_string()],
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };

        // Act
        let (expanded, notes) = expand_reverse_dependencies(&config, &["a".to_string()]);

        // Assert
        let mut sorted = expanded.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["a", "b", "c"]);
        assert_eq!(notes.get("b").unwrap(), "depends on a");
        assert_eq!(notes.get("c").unwrap(), "depends on b");
    }

    #[test]
    fn test_expand_reverse_dependencies_no_note_for_requested() {
        // Arrange — editor depends on git; both requested explicitly
        let config = Config {
            packages: std::collections::BTreeMap::from([
                (
                    "editor".to_string(),
                    PackageConfig {
                        depends_on: vec!["git".to_string()],
                        ..Default::default()
                    },
                ),
                ("git".to_string(), PackageConfig::default()),
            ]),
            ..Default::default()
        };

        // Act
        let (_, notes) =
            expand_reverse_dependencies(&config, &["git".to_string(), "editor".to_string()]);

        // Assert — neither gets a note since both were explicitly requested
        assert!(notes.is_empty());
    }

    // --- Reverse dependency ordering integration tests ---

    #[test]
    fn test_uninstall_reverse_deps_included_in_plan() {
        // Arrange — editor depends on git; uninstall git should also uninstall editor
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  editor:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["editor", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["editor".to_string(), "git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — only request git, editor should be pulled in as reverse dependency
        run_action(
            &ctx,
            &["git".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — editor uninstalled before git (dependent first)
        let written = String::from_utf8(output).unwrap();
        let editor_pos = written
            .find("Uninstalling editor")
            .expect("editor should be uninstalled");
        let git_pos = written
            .find("Uninstalling git")
            .expect("git should be uninstalled");
        assert!(
            editor_pos < git_pos,
            "editor (dependent) must be uninstalled before git (dependency)"
        );
        // Both markers should exist
        assert!(marker_dir.path().join("editor_marker").exists());
        assert!(marker_dir.path().join("git_marker").exists());
    }

    #[test]
    fn test_uninstall_reverse_deps_shows_depends_on_note() {
        // Arrange — editor depends on git; uninstall git should show note for editor
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  editor:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["editor", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["editor".to_string(), "git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["git".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — plan display shows "depends on git" for editor
        let written = String::from_utf8(output).unwrap();
        assert!(
            written.contains("editor (depends on git)"),
            "Plan should show 'depends on git' annotation for editor. Got: {written}"
        );
    }

    #[test]
    fn test_uninstall_reverse_deps_transitive_chain() {
        // Arrange — c depends on b, b depends on a; uninstall a should include b and c
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  a: {}\n  b:\n    depends_on:\n      - a\n  c:\n    depends_on:\n      - b\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["a", "b", "c"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — only request a, b and c should be pulled in as reverse deps
        run_action(
            &ctx,
            &["a".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — c before b before a (dependents first)
        let written = String::from_utf8(output).unwrap();
        let c_pos = written.find("Uninstalling c").unwrap();
        let b_pos = written.find("Uninstalling b").unwrap();
        let a_pos = written.find("Uninstalling a").unwrap();
        assert!(c_pos < b_pos, "c must be uninstalled before b");
        assert!(b_pos < a_pos, "b must be uninstalled before a");
    }

    #[test]
    fn test_uninstall_reverse_deps_skips_not_installed() {
        // Arrange — editor depends on git; editor is NOT installed
        let marker_dir = TempDir::new().unwrap();
        let git_marker = marker_dir.path().join("git_marker");
        let yaml = "packages:\n  editor:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        let git_dir = ctx.packages_dir().join("git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join(format!("uninstall.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", git_marker.display()),
        )
        .unwrap();
        // Only git is installed, editor is not
        let state = State {
            installed: vec!["git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["git".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — editor shows as not installed in skip section, git is uninstalled
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling git...\ndone"));
        assert!(written.contains("editor (not installed"));
        assert!(git_marker.exists());
    }

    #[test]
    fn test_uninstall_reverse_deps_removed_from_state() {
        // Arrange — editor depends on git; both installed; uninstall git
        let marker_dir = TempDir::new().unwrap();
        let yaml = "packages:\n  editor:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        let ext = script_extension();
        for pkg in ["editor", "git"] {
            let marker_path = marker_dir.path().join(format!("{pkg}_marker"));
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
            )
            .unwrap();
        }
        let state = State {
            installed: vec!["editor".to_string(), "git".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["git".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — both removed from state
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(!state.installed.contains(&"editor".to_string()));
        assert!(!state.installed.contains(&"git".to_string()));
    }

    // --- Circular dependency graceful handling tests ---

    #[test]
    fn test_install_skips_circular_dependency_packages() {
        // Arrange — a and b form a cycle, c has no deps
        let (_tmp, ctx) =
            fixture("packages:\n  a:\n    depends_on: [b]\n  b:\n    depends_on: [a]\n  c: {}\n");
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("c_installed");
        let pkg_dir = ctx.packages_dir().join("c");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["a".to_string(), "b".to_string(), "c".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — c should be installed, a and b skipped
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(written.contains("will be skipped"));
        assert!(marker_path.exists(), "c should have been installed");
    }

    #[test]
    fn test_install_all_circular_shows_nothing_to_do() {
        // Arrange — a and b form a cycle, no other packages requested
        let (_tmp, ctx) =
            fixture("packages:\n  a:\n    depends_on: [b]\n  b:\n    depends_on: [a]\n");
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["a".to_string(), "b".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_uninstall_skips_circular_dependency_packages() {
        // Arrange — a and b form a cycle, both installed
        let (_tmp, ctx) =
            fixture("packages:\n  a:\n    depends_on: [b]\n  b:\n    depends_on: [a]\n  c: {}\n");
        // Mark all as installed
        let state = State {
            installed: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        // Create uninstall script for c
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("c_uninstalled");
        let pkg_dir = ctx.packages_dir().join("c");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            pkg_dir.join(format!("uninstall.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["a".to_string(), "b".to_string(), "c".to_string()],
            Action::Uninstall,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — c should be uninstalled, a and b skipped
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(marker_path.exists(), "c should have been uninstalled");
    }

    #[test]
    fn test_apply_skips_circular_dependency_packages() {
        // Arrange — a and b form a cycle (not installed), c has no deps (not installed)
        let (_tmp, ctx) =
            fixture("packages:\n  a:\n    depends_on: [b]\n  b:\n    depends_on: [a]\n  c: {}\n");
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("c_installed");
        let pkg_dir = ctx.packages_dir().join("c");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", marker_path.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert — c should be installed, a and b skipped due to cycle
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("a (circular dependency)"));
        assert!(written.contains("b (circular dependency)"));
        assert!(marker_path.exists(), "c should have been installed");
    }

    #[test]
    fn test_apply_renders_single_skipped_header_when_dep_disabled() {
        // Arrange — neovim (enabled) depends on git (disabled). expand_dependencies
        // would naively pull git into expanded_install, and pre-fix that produced two
        // consecutive "The following packages will be skipped:" headers (one from
        // plan.display(), one from apply_to's top-level disabled section).
        let yaml = "packages:\n  neovim:\n    depends_on: [git]\n  git:\n    enabled: false\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert — only one skipped header, both entries present, no duplicate listing of git
        let written = String::from_utf8(output).unwrap();
        let header_count = written
            .matches("The following packages will be skipped:")
            .count();
        assert_eq!(
            header_count, 1,
            "expected exactly one skipped header, got {header_count}; output:\n{written}"
        );
        assert!(written.contains("git (disabled)"));
        assert!(written.contains("neovim (dependency disabled: git)"));
        // git should appear only once in the skipped section, not duplicated
        assert_eq!(
            written.matches("git (disabled)").count(),
            1,
            "git (disabled) should appear exactly once, output:\n{written}"
        );
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_apply_orders_skipped_after_install_and_update() {
        // Arrange — neovim install (enabled+not-in-state), ripgrep update (enabled+in-state),
        // docker disabled. The consolidated skipped section must appear AFTER both the
        // install and update sections (matching README's installed → updated → skipped).
        let marker_dir = TempDir::new().unwrap();
        let neo_marker = marker_dir.path().join("neo_install");
        let rg_marker = marker_dir.path().join("rg_update");
        let yaml = "packages:\n  docker:\n    enabled: false\n  neovim: {}\n  ripgrep: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", &neo_marker);
        write_script(&ctx, "ripgrep", "update", &rg_marker);
        State {
            installed: vec!["ripgrep".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert — sections in canonical order: installed, updated, skipped
        let written = String::from_utf8(output).unwrap();
        let install_pos = written
            .find("The following packages will be installed:")
            .expect("missing installed section");
        let update_pos = written
            .find("The following packages will be updated:")
            .expect("missing updated section");
        let skipped_pos = written
            .find("The following packages will be skipped:")
            .expect("missing skipped section");
        assert!(
            install_pos < update_pos,
            "installed should come before updated; output:\n{written}"
        );
        assert!(
            update_pos < skipped_pos,
            "updated should come before skipped; output:\n{written}"
        );
        // Only one skipped header
        assert_eq!(
            written
                .matches("The following packages will be skipped:")
                .count(),
            1
        );
    }

    #[test]
    fn test_apply_skipped_section_orders_disabled_before_dependency_disabled() {
        // Arrange — neovim (enabled) depends on git (disabled). In the consolidated
        // skipped section, `(disabled)` must come before `(dependency disabled:)` to
        // match the COMMAND_OUTPUT.md Plan Display order.
        let yaml = "packages:\n  neovim:\n    depends_on: [git]\n  git:\n    enabled: false\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, false, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        let disabled_pos = written
            .find("git (disabled)")
            .expect("missing git (disabled) entry");
        let dep_disabled_pos = written
            .find("neovim (dependency disabled: git)")
            .expect("missing neovim (dependency disabled) entry");
        assert!(
            disabled_pos < dep_disabled_pos,
            "(disabled) should come before (dependency disabled:); output:\n{written}"
        );
    }

    #[test]
    fn test_run_action_dry_run_install_displays_plan_without_executing() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            true,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be installed:"));
        assert!(written.contains("neovim"));
        assert!(!written.contains("Proceed?"));
        assert!(!written.contains("Installing"));
        assert!(!written.contains("Aborted"));
        assert!(!marker_path.exists(), "install script must not run");
    }

    #[test]
    fn test_run_action_dry_run_update_displays_plan_without_executing() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            true,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be updated:"));
        assert!(!written.contains("Proceed?"));
        assert!(!written.contains("Updating"));
        assert!(!marker_path.exists(), "update script must not run");
    }

    #[test]
    fn test_run_action_dry_run_uninstall_displays_plan_without_executing() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            &marker_path,
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
            true,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be uninstalled:"));
        assert!(!written.contains("Proceed?"));
        assert!(!written.contains("Uninstalling"));
        assert!(!marker_path.exists(), "uninstall script must not run");
        // state.yml must remain unchanged
        let state_after = State::load(&ctx.state_path()).unwrap();
        assert!(state_after.installed.contains(&"neovim".to_string()));
    }

    #[test]
    fn test_run_action_dry_run_does_not_update_state() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            true,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — state.yml should not exist since nothing was installed
        assert!(
            !ctx.state_path().exists(),
            "state.yml must not be created during dry-run"
        );
    }

    #[test]
    fn test_run_action_dry_run_shows_nothing_to_do_for_empty_plan() {
        // Arrange — package is disabled, so plan is empty
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            true,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — empty-plan path still runs; Nothing to do. is shown
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Nothing to do."));
        assert!(!written.contains("Proceed?"));
    }

    #[test]
    fn test_apply_dry_run_displays_plan_without_executing() {
        // Arrange
        let marker_dir = TempDir::new().unwrap();
        let marker_path = marker_dir.path().join("should_not_run");
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            &marker_path,
        );
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following packages will be installed:"));
        assert!(written.contains("neovim"));
        assert!(!written.contains("Proceed?"));
        assert!(!written.contains("Installing"));
        assert!(!marker_path.exists(), "install script must not run");
        assert!(
            !ctx.state_path().exists(),
            "state.yml must not be created during dry-run"
        );
    }

    #[test]
    fn test_install_skips_package_with_disabled_direct_dependency() {
        // Arrange — neovim (enabled) depends on git (disabled)
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_should_not_run");
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on: [git]\n  git:\n    enabled: false\n");
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — neovim is in the skipped section with "dependency disabled: git"
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (dependency disabled: git)"));
        assert!(written.contains("git (disabled)"));
        assert!(written.contains("Nothing to do."));
        assert!(!written.contains("Installing neovim"));
        assert!(!neovim_marker.exists());
    }

    #[test]
    fn test_install_propagates_disabled_dep_transitively() {
        // Arrange — neovim (enabled) → git (enabled) → curl (disabled)
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_should_not_run");
        let git_marker = marker_dir.path().join("git_should_not_run");
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    depends_on: [git]\n  git:\n    depends_on: [curl]\n  curl:\n    enabled: false\n",
        );
        let ext = script_extension();
        for (name, marker) in [("neovim", &neovim_marker), ("git", &git_marker)] {
            let pkg_dir = ctx.packages_dir().join(name);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            std::fs::write(
                pkg_dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\ntouch '{}'\n", marker.display()),
            )
            .unwrap();
        }
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — each blames its most direct unavailable dep
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim (dependency disabled: git)"));
        assert!(written.contains("git (dependency disabled: curl)"));
        assert!(written.contains("curl (disabled)"));
        assert!(!neovim_marker.exists());
        assert!(!git_marker.exists());
    }

    #[test]
    fn test_update_unaffected_by_disabled_dependency() {
        // Arrange — neovim (enabled, in state) depends on git (disabled, in state)
        let marker_dir = TempDir::new().unwrap();
        let neovim_marker = marker_dir.path().join("neovim_update_marker");
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    depends_on: [git]\n  git:\n    enabled: false\n");
        State {
            installed: vec!["neovim".to_string(), "git".to_string()],
        }
        .save(&ctx.state_path())
        .unwrap();
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            pkg_dir.join(format!("update.{ext}")),
            format!("#!/usr/bin/env sh\ntouch '{}'\n", neovim_marker.display()),
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            false,
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — neovim runs the update; git is shown as disabled but not blamed
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim...\ndone"));
        assert!(!written.contains("dependency disabled"));
        assert!(neovim_marker.exists());
    }
}
