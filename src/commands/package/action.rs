use crate::config::{Config, PackageConfig};
use crate::context::Context;
use crate::plan::{Action, Plan, confirm_plan};
use crate::state::State;
use crate::topo::topological_sort;
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::Path;

pub fn apply(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    apply_to(ctx, &mut std::io::stdin().lock(), &mut std::io::stdout())
}

pub(crate) fn apply_to<R: BufRead, W: Write>(
    ctx: &Context,
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
        for name in &disabled_packages {
            writeln!(writer, "Skipping {name} (disabled)")?;
        }
        writeln!(writer, "Nothing to do.")?;
        return Ok(());
    }

    // Expand install dependencies — some deps may already be installed (update targets)
    let expanded_install = if !to_install.is_empty() {
        expand_dependencies(&config, &to_install)
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
    let ordered = topological_sort(&config, &all_packages)?;

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

    let install_plan = if !install_names.is_empty() {
        Some(Plan::build(&config, &install_names, Action::Install, &installed)?)
    } else {
        None
    };
    let update_plan = if !update_names.is_empty() {
        Some(Plan::build(&config, &update_names, Action::Update, &installed)?)
    } else {
        None
    };

    // Display combined plan
    if let Some(ref plan) = install_plan {
        let display = plan.display();
        if !display.is_empty() {
            writeln!(writer, "{display}")?;
        }
    }
    if let Some(ref plan) = update_plan {
        let display = plan.display();
        if !display.is_empty() {
            writeln!(writer, "{display}")?;
        }
    }
    for name in &disabled_packages {
        writeln!(writer, "Skipping {name} (disabled)")?;
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
        write!(writer, "{verb} {name}... ")?;
        writer.flush()?;

        match execute_script(&script_path) {
            Ok(_) => {
                writeln!(writer, "done")?;
                update_state_per_package(ctx, *action, name)?;
            }
            Err(e) => {
                writeln!(writer, "FAILED")?;
                writeln!(writer, "Error: {e}")?;
                had_errors = true;
            }
        }
    }

    if had_errors {
        Err("Some packages failed".into())
    } else {
        Ok(())
    }
}

pub fn install(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    run_action(
        ctx,
        packages,
        Action::Install,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub fn update(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    run_action(
        ctx,
        packages,
        Action::Update,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub fn uninstall(
    ctx: &Context,
    packages: &[String],
    all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    uninstall_to(
        ctx,
        packages,
        all,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub(crate) fn uninstall_to<R: BufRead, W: Write>(
    ctx: &Context,
    packages: &[String],
    all: bool,
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

    run_action(ctx, &resolved_packages, Action::Uninstall, reader, writer)
}

/// Execute an action for the given packages, with confirmation prompt.
/// I/O is injectable for testability. Reusable for install/update/uninstall.
pub fn run_action<R: BufRead, W: Write>(
    ctx: &Context,
    packages: &[String],
    action: Action,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    // For install, expand to include transitive dependencies and sort topologically
    let ordered_packages = if action == Action::Install {
        let expanded = expand_dependencies(&config, packages);
        topological_sort(&config, &expanded)?
    } else {
        packages.to_vec()
    };

    let state_path = ctx.state_path();
    let installed = if state_path.exists() {
        State::load(&state_path)?.installed
    } else {
        Vec::new()
    };

    let plan = Plan::build(&config, &ordered_packages, action, &installed)?;

    if plan.is_empty() {
        let display = plan.display();
        if !display.is_empty() {
            writeln!(writer, "{display}")?;
        }
        writeln!(writer, "No packages to {action}.")?;
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

        write!(writer, "{verb} {name}... ")?;
        writer.flush()?;

        match execute_script(&script_path) {
            Ok(_) => {
                writeln!(writer, "done")?;
                update_state_per_package(ctx, action, name)?;
            }
            Err(e) => {
                writeln!(writer, "FAILED")?;
                writeln!(writer, "Error: {e}")?;
                had_errors = true;
            }
        }
    }

    if had_errors {
        Err("Some packages failed".into())
    } else {
        Ok(())
    }
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

/// Resolve the script filename for a given action, considering overrides.
fn resolve_script_name(pkg_config: &PackageConfig, action: Action) -> String {
    let action_str = action.as_str();
    let resolved_action = pkg_config
        .actions_overrides
        .get(action_str)
        .map(|s| s.as_str())
        .unwrap_or(action_str);
    let ext = super::script_extension();
    format!("{resolved_action}.{ext}")
}

/// Execute a script file via the OS-appropriate shell. Returns the output.
fn execute_script(script_path: &Path) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let output = std::process::Command::new(super::shell_command())
        .arg(script_path)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Script failed: {stderr}").into());
    }
    Ok(output)
}

/// Expand a list of packages to include all transitive dependencies.
/// Returns the expanded set as a Vec in no particular order.
fn expand_dependencies(config: &Config, packages: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = packages.to_vec();

    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        result.push(name.clone());
        if let Some(pkg_config) = config.packages.get(&name) {
            for dep in &pkg_config.depends_on {
                if !visited.contains(dep) {
                    stack.push(dep.clone());
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::package::script_extension;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let base_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(base_dir), "default".to_string());

        std::fs::create_dir_all(ctx.config_path().parent().unwrap()).unwrap();
        std::fs::write(ctx.config_path(), yaml).unwrap();

        (tmp, ctx)
    }

    fn fixture_with_script(
        yaml: &str,
        pkg: &str,
        action: &str,
        marker: &str,
    ) -> (TempDir, Context) {
        let (tmp, ctx) = fixture(yaml);
        let pkg_dir = ctx.packages_dir().join(pkg);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let script_path = pkg_dir.join(format!("{action}.{ext}"));
        std::fs::write(
            &script_path,
            format!("#!/usr/bin/env sh\necho '{marker}'\n"),
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
    fn test_resolve_script_name_with_override() {
        // Arrange
        let pkg_config = PackageConfig {
            actions_overrides: std::collections::BTreeMap::from([(
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
    fn test_execute_script_captures_output() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let script_path = tmp.path().join("test.sh");
        std::fs::write(&script_path, "#!/usr/bin/env sh\necho 'MARKER_XYZ'\n").unwrap();

        // Act
        let sut = execute_script(&script_path).unwrap();

        // Assert
        let stdout = String::from_utf8(sut.stdout).unwrap();
        assert!(stdout.contains("MARKER_XYZ"));
    }

    #[test]
    fn test_run_action_executes_install_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "INSTALL_MARKER",
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim... done"));
    }

    #[test]
    fn test_run_action_skips_disabled_packages() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "install",
            "SHOULD_NOT_RUN",
        );
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (disabled)"));
        assert!(written.contains("No packages to install."));
        assert!(!written.contains("Installing"));
    }

    #[test]
    fn test_run_action_aborts_on_no_confirmation() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "SHOULD_NOT_RUN",
        );
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Installing"));
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Some packages failed")
        );
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Script not found"));
    }

    #[test]
    fn test_run_action_respects_action_overrides() {
        // Arrange
        let (_tmp, ctx) =
            fixture("packages:\n  neovim:\n    actions_overrides:\n      update: install\n");
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
            "#!/usr/bin/env sh\necho 'OVERRIDE_MARKER'\n",
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim... done"));
    }

    #[test]
    fn test_run_action_executes_update_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            "UPDATE_MARKER",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim... done"));
    }

    #[test]
    fn test_run_action_skips_disabled_packages_for_update() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "update",
            "SHOULD_NOT_RUN",
        );
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (disabled)"));
        assert!(written.contains("No packages to update."));
        assert!(!written.contains("Updating"));
    }

    #[test]
    fn test_run_action_aborts_update_on_no_confirmation() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            "SHOULD_NOT_RUN",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Updating"));
    }

    #[test]
    fn test_run_action_executes_uninstall_scripts() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim... done"));
    }

    #[test]
    fn test_run_action_executes_disabled_packages_for_uninstall() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "uninstall",
            "echo UNINSTALL_MARKER",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(!written.contains("Skipping neovim (disabled)"));
        assert!(written.contains("Uninstalling neovim"));
    }

    #[test]
    fn test_run_action_aborts_uninstall_on_no_confirmation() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "SHOULD_NOT_RUN",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Uninstalling"));
    }

    #[test]
    fn test_run_action_executes_multiple_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}_MARKER'\n"),
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim... done"));
        assert!(written.contains("Installing ripgrep... done"));
    }

    #[test]
    fn test_install_records_package_in_state() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "INSTALL_MARKER",
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "INSTALL_MARKER",
        );
        assert!(!ctx.state_path().exists());
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  ripgrep: {}\n",
            "neovim",
            "install",
            "INSTALL_MARKER",
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "INSTALL_MARKER",
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
        );
        assert!(!ctx.state_path().exists());
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Uninstall,
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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}_UNINSTALL'\n"),
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            "UPDATE_MARKER",
        );
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Update,
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "install",
            "SHOULD_NOT_RUN",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (already installed)"));
        assert!(written.contains("No packages to install."));
        assert!(!written.contains("Installing"));
    }

    #[test]
    fn test_install_skips_already_installed_but_installs_new() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  zed: {}\n",
            "neovim",
            "install",
            "NEOVIM_MARKER",
        );
        // Also create install script for zed
        let zed_dir = ctx.packages_dir().join("zed");
        std::fs::create_dir_all(&zed_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            zed_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'ZED_MARKER'\n",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (already installed)"));
        assert!(written.contains("Installing zed... done"));
        assert!(!written.contains("Installing neovim"));
    }

    #[test]
    fn test_install_all_already_installed_shows_no_packages() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n  zed: {}\n",
            "neovim",
            "install",
            "SHOULD_NOT_RUN",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (already installed)"));
        assert!(written.contains("Skipping zed (already installed)"));
        assert!(written.contains("No packages to install."));
    }

    #[test]
    fn test_install_records_state_per_package_on_partial_failure() {
        // Arrange: neovim has a valid script, ripgrep has no script (will fail)
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        let ext = script_extension();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'NEOVIM_MARKER'\n",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim... done"));
        assert!(written.contains("Script not found"));
    }

    #[test]
    fn test_install_continues_after_script_failure() {
        // Arrange: neovim has a failing script, ripgrep has a valid script
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
            "#!/usr/bin/env sh\necho 'RIPGREP_MARKER'\n",
        )
        .unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["neovim".to_string(), "ripgrep".to_string()],
            Action::Install,
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim... FAILED"));
        assert!(written.contains("Installing ripgrep... done"));
    }

    #[test]
    fn test_uninstall_records_state_per_package() {
        // Arrange: two packages with valid uninstall scripts
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ext = script_extension();
        for pkg in &["neovim", "ripgrep"] {
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}_UNINSTALL'\n"),
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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("uninstall.{ext}")),
            "#!/usr/bin/env sh\necho 'NEOVIM_UNINSTALL'\n",
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
            &mut input,
            &mut output,
        );

        // Assert
        assert!(result.is_err());
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["ripgrep"]);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim... done"));
        assert!(written.contains("Script not found"));
    }

    #[test]
    fn test_uninstall_disables_package_in_config() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}_UNINSTALL'\n"),
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim:\n    enabled: false\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            "UPDATE_MARKER",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim... done"));
        assert!(!written.contains("already installed"));
    }

    #[test]
    fn test_uninstall_all_uninstalls_all_installed_packages() {
        // Arrange
        let (_tmp, ctx) = fixture("packages:\n  neovim: {}\n  ripgrep: {}\n");
        for pkg in &["neovim", "ripgrep"] {
            let pkg_dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&pkg_dir).unwrap();
            let ext = script_extension();
            std::fs::write(
                pkg_dir.join(format!("uninstall.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}_UNINSTALL'\n"),
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
        super::uninstall_to(&ctx, &[], true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim... done"));
        assert!(written.contains("Uninstalling ripgrep... done"));
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
        super::uninstall_to(&ctx, &[], true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("No packages to uninstall."));
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
        super::uninstall_to(&ctx, &[], true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("No packages to uninstall."));
    }

    #[test]
    fn test_uninstall_all_shows_confirmation_prompt() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("will be uninstalled"));
        assert!(written.contains("neovim"));
        assert!(written.contains("Proceed? [y/N]"));
        assert!(written.contains("Aborted."));
    }

    #[test]
    fn test_update_loads_state_for_plan() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "update",
            "UPDATE_MARKER",
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
            &mut input,
            &mut output,
        );

        // Assert — state is loaded but update still executes in-state packages
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim... done"));
    }

    #[test]
    fn test_uninstall_loads_state_for_plan() {
        // Arrange
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
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
            &mut input,
            &mut output,
        );

        // Assert — state is loaded but uninstall still executes in-state packages
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim... done"));
    }

    #[test]
    fn test_uninstall_all_ignores_packages_arg() {
        // Arrange: --all flag set, packages arg is empty, state has packages
        let (_tmp, ctx) = fixture_with_script(
            "packages:\n  neovim: {}\n",
            "neovim",
            "uninstall",
            "UNINSTALL_MARKER",
        );
        let state = State {
            installed: vec!["neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        super::uninstall_to(&ctx, &[], true, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Uninstalling neovim... done"));
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
        };

        // Act
        let sut = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert
        assert_eq!(sut, vec!["neovim"]);
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
        };

        // Act
        let sut = expand_dependencies(&config, &["neovim".to_string()]);

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
        };

        // Act
        let sut = expand_dependencies(&config, &["neovim".to_string(), "zed".to_string()]);

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
        };

        // Act
        let sut = expand_dependencies(&config, &["neovim".to_string()]);

        // Assert — unknown_pkg is included (Plan::build will error on it)
        let mut sorted = sut.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["neovim", "unknown_pkg"]);
    }

    // --- Dependency ordering integration tests ---

    #[test]
    fn test_install_includes_dependencies_in_order() {
        // Arrange — neovim depends on git; only request neovim
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        // Create install scripts for both packages
        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'NEOVIM_INSTALL'\n",
        )
        .unwrap();

        let git_dir = ctx.packages_dir().join("git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            git_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'GIT_INSTALL'\n",
        )
        .unwrap();

        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act — only request neovim, git should be pulled in as dependency
        run_action(
            &ctx,
            &["neovim".to_string()],
            Action::Install,
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
    }

    #[test]
    fn test_install_dependencies_recorded_in_state() {
        // Arrange — neovim depends on git; only request neovim
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        for pkg in ["neovim", "git"] {
            let dir = ctx.packages_dir().join(pkg);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("install.{ext}")),
                format!("#!/usr/bin/env sh\necho '{pkg}'\n"),
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
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'NEOVIM'\n",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — git skipped as already installed, neovim installed
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping git (already installed)"));
        assert!(written.contains("Installing neovim... done"));
    }

    #[test]
    fn test_install_circular_dependency_errors() {
        // Arrange — a depends on b, b depends on a
        let yaml =
            "packages:\n  a:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(
            &ctx,
            &["a".to_string()],
            Action::Install,
            &mut input,
            &mut output,
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
    fn test_update_does_not_expand_dependencies() {
        // Arrange — neovim depends on git; only request neovim for update
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);

        let ext = script_extension();
        let neovim_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&neovim_dir).unwrap();
        std::fs::write(
            neovim_dir.join(format!("update.{ext}")),
            "#!/usr/bin/env sh\necho 'UPDATE'\n",
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
            &mut input,
            &mut output,
        )
        .unwrap();

        // Assert — only neovim updated, git not mentioned
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating neovim... done"));
        assert!(!written.contains("git"));
    }

    // --- apply_to tests ---

    fn write_script(ctx: &Context, pkg: &str, action: &str, marker: &str) {
        let pkg_dir = ctx.packages_dir().join(pkg);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let script_path = pkg_dir.join(format!("{action}.{ext}"));
        std::fs::write(
            &script_path,
            format!("#!/usr/bin/env sh\necho '{marker}'\n"),
        )
        .unwrap();
    }

    #[test]
    fn test_apply_installs_enabled_not_in_state_and_updates_enabled_in_state() {
        // Arrange: neovim is enabled+not-in-state (install), zed is enabled+in-state (update)
        let yaml = "packages:\n  neovim: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", "NEO_INSTALL");
        write_script(&ctx, "zed", "update", "ZED_UPDATE");
        let state = State { installed: vec!["zed".to_string()] };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing neovim... done"));
        assert!(written.contains("Updating zed... done"));
        let state = State::load(&ctx.state_path()).unwrap();
        assert!(state.installed.contains(&"neovim".to_string()));
        assert!(state.installed.contains(&"zed".to_string()));
    }

    #[test]
    fn test_apply_skips_disabled_packages() {
        // Arrange: neovim is disabled, zed is enabled+not-in-state
        let yaml = "packages:\n  neovim:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", "ZED_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing zed... done"));
        assert!(written.contains("Skipping neovim (disabled)"));
        assert!(!written.contains("Installing neovim"));
    }

    #[test]
    fn test_apply_nothing_to_do_when_all_disabled() {
        // Arrange: all packages disabled
        let yaml = "packages:\n  neovim:\n    enabled: false\n";
        let (_tmp, ctx) = fixture(yaml);
        let mut input = std::io::Cursor::new(b"".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (disabled)"));
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
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Nothing to do."));
    }

    #[test]
    fn test_apply_aborts_on_no_confirmation() {
        // Arrange
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", "SHOULD_NOT_RUN");
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Installing"));
    }

    #[test]
    fn test_apply_only_installs_when_nothing_in_state() {
        // Arrange: two enabled packages, no state
        let yaml = "packages:\n  git: {}\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "install", "GIT_INSTALL");
        write_script(&ctx, "neovim", "install", "NEO_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Installing git... done"));
        assert!(written.contains("Installing neovim... done"));
        assert!(!written.contains("Updating"));
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed.len(), 2);
    }

    #[test]
    fn test_apply_only_updates_when_all_in_state() {
        // Arrange: two enabled packages, both in state
        let yaml = "packages:\n  git: {}\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", "GIT_UPDATE");
        write_script(&ctx, "neovim", "update", "NEO_UPDATE");
        let state = State { installed: vec!["git".to_string(), "neovim".to_string()] };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Updating git... done"));
        assert!(written.contains("Updating neovim... done"));
        assert!(!written.contains("Installing"));
    }

    #[test]
    fn test_apply_records_installed_in_state() {
        // Arrange: neovim enabled+not-in-state
        let yaml = "packages:\n  neovim: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", "INSTALL_NEO");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let state = State::load(&ctx.state_path()).unwrap();
        assert_eq!(state.installed, vec!["neovim"]);
    }

    #[test]
    fn test_apply_shows_combined_plan() {
        // Arrange: neovim to install, zed to update
        let yaml = "packages:\n  neovim: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "neovim", "install", "NEO_INSTALL");
        write_script(&ctx, "zed", "update", "ZED_UPDATE");
        let state = State { installed: vec!["zed".to_string()] };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

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
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", "GIT_UPDATE");
        write_script(&ctx, "neovim", "install", "NEO_INSTALL");
        let state = State { installed: vec!["git".to_string()] };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert: git updated before neovim installed
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Updating git... done").unwrap();
        let neo_pos = written.find("Installing neovim... done").unwrap();
        assert!(git_pos < neo_pos, "git should be updated before neovim is installed");
    }

    #[test]
    fn test_apply_topological_order_for_install_chain() {
        // Arrange: c depends on b, b depends on a. All are new installs.
        let yaml = "packages:\n  c:\n    depends_on:\n      - b\n  b:\n    depends_on:\n      - a\n  a: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "a", "install", "A_INSTALL");
        write_script(&ctx, "b", "install", "B_INSTALL");
        write_script(&ctx, "c", "install", "C_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert: a before b before c
        let written = String::from_utf8(output).unwrap();
        let a_pos = written.find("Installing a... done").unwrap();
        let b_pos = written.find("Installing b... done").unwrap();
        let c_pos = written.find("Installing c... done").unwrap();
        assert!(a_pos < b_pos, "a should be installed before b");
        assert!(b_pos < c_pos, "b should be installed before c");
    }

    #[test]
    fn test_apply_topological_order_for_updates() {
        // Arrange: neovim depends on git. Both are already installed (both update).
        // git should be updated before neovim.
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "update", "GIT_UPDATE");
        write_script(&ctx, "neovim", "update", "NEO_UPDATE");
        let state = State {
            installed: vec!["git".to_string(), "neovim".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert: git updated before neovim
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Updating git... done").unwrap();
        let neo_pos = written.find("Updating neovim... done").unwrap();
        assert!(git_pos < neo_pos, "git should be updated before neovim");
    }

    #[test]
    fn test_apply_expands_transitive_deps_for_install() {
        // Arrange: neovim depends on git (not in config as enabled but is a dep).
        // git is not in state — should be pulled in as an install dependency.
        let yaml = "packages:\n  neovim:\n    depends_on:\n      - git\n  git: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "git", "install", "GIT_INSTALL");
        write_script(&ctx, "neovim", "install", "NEO_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert: both installed, git first
        let written = String::from_utf8(output).unwrap();
        let git_pos = written.find("Installing git... done").unwrap();
        let neo_pos = written.find("Installing neovim... done").unwrap();
        assert!(git_pos < neo_pos, "git should be installed before neovim");
    }

    #[test]
    fn test_apply_mixed_install_update_diamond_dependency() {
        // Arrange: d depends on b and c, b and c depend on a.
        // a and b are already installed (update), c and d are new (install).
        let yaml = "packages:\n  d:\n    depends_on:\n      - b\n      - c\n  c:\n    depends_on:\n      - a\n  b:\n    depends_on:\n      - a\n  a: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "a", "update", "A_UPDATE");
        write_script(&ctx, "b", "update", "B_UPDATE");
        write_script(&ctx, "c", "install", "C_INSTALL");
        write_script(&ctx, "d", "install", "D_INSTALL");
        let state = State {
            installed: vec!["a".to_string(), "b".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert: a before b and c, b and c before d
        let written = String::from_utf8(output).unwrap();
        let a_pos = written.find("Updating a... done").unwrap();
        let b_pos = written.find("Updating b... done").unwrap();
        let c_pos = written.find("Installing c... done").unwrap();
        let d_pos = written.find("Installing d... done").unwrap();
        assert!(a_pos < c_pos, "a should be updated before c is installed");
        assert!(a_pos < b_pos, "a should be updated before b is updated");
        assert!(b_pos < d_pos, "b should be updated before d is installed");
        assert!(c_pos < d_pos, "c should be installed before d is installed");
    }

    #[test]
    fn test_apply_shows_disabled_in_plan_with_enabled_packages() {
        // Arrange: neovim disabled, zed enabled+not-in-state, ripgrep enabled+in-state
        let yaml =
            "packages:\n  neovim:\n    enabled: false\n  ripgrep: {}\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", "ZED_INSTALL");
        write_script(&ctx, "ripgrep", "update", "RG_UPDATE");
        let state = State {
            installed: vec!["ripgrep".to_string()],
        };
        state.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping neovim (disabled)"));
        assert!(written.contains("Installing zed... done"));
        assert!(written.contains("Updating ripgrep... done"));
    }

    #[test]
    fn test_apply_shows_multiple_disabled_packages() {
        // Arrange: two disabled packages, one enabled
        let yaml = "packages:\n  docker:\n    enabled: false\n  neovim:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", "ZED_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Skipping docker (disabled)"));
        assert!(written.contains("Skipping neovim (disabled)"));
        assert!(written.contains("Installing zed... done"));
    }

    #[test]
    fn test_apply_disabled_shown_before_prompt() {
        // Arrange: disabled package should appear in plan before confirmation prompt
        let yaml = "packages:\n  docker:\n    enabled: false\n  zed: {}\n";
        let (_tmp, ctx) = fixture(yaml);
        write_script(&ctx, "zed", "install", "ZED_INSTALL");
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        apply_to(&ctx, &mut input, &mut output).unwrap();

        // Assert
        let written = String::from_utf8(output).unwrap();
        let skip_pos = written.find("Skipping docker (disabled)").unwrap();
        let prompt_pos = written.find("Proceed? [y/N]").unwrap();
        assert!(
            skip_pos < prompt_pos,
            "disabled message should appear before confirmation prompt"
        );
    }
}
