use crate::config::{Config, PackageConfig};
use crate::plan::{confirm_plan, Action, Plan};
use crate::context::Context;
use crate::state::State;
use std::io::{BufRead, Write};
use std::path::Path;

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

    let state_path = ctx.state_path();
    let installed = if state_path.exists() {
        State::load(&state_path)?.installed
    } else {
        Vec::new()
    };

    let plan = Plan::build(&config, packages, action, &installed)?;

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

pub fn uninstall(ctx: &Context, packages: &[String], all: bool) -> Result<(), Box<dyn std::error::Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::package::script_extension;
    use tempfile::TempDir;

    fn fixture(yaml: &str) -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let base_dir = tmp.path().to_path_buf();
        let ctx = Context::new(Some(base_dir));

        std::fs::create_dir_all(ctx.config_path().parent().unwrap()).unwrap();
        std::fs::write(ctx.config_path(), yaml).unwrap();

        (tmp, ctx)
    }

    fn fixture_with_script(yaml: &str, pkg: &str, action: &str, marker: &str) -> (TempDir, Context) {
        let (tmp, ctx) = fixture(yaml);
        let pkg_dir = ctx.packages_dir().join(pkg);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        let script_path = pkg_dir.join(format!("{action}.{ext}"));
        std::fs::write(&script_path, format!("#!/usr/bin/env sh\necho '{marker}'\n")).unwrap();
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
            actions_overrides: std::collections::BTreeMap::from([
                ("update".to_string(), "install".to_string()),
            ]),
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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output);

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output);

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output);

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Some packages failed"));
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Script not found"));
    }

    #[test]
    fn test_run_action_respects_action_overrides() {
        // Arrange
        let (_tmp, ctx) = fixture(
            "packages:\n  neovim:\n    actions_overrides:\n      update: install\n",
        );
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let pkg_dir = ctx.packages_dir().join("neovim");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        let ext = script_extension();
        // Only create install script (update is overridden to install)
        std::fs::write(
            pkg_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'OVERRIDE_MARKER'\n",
        ).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output);

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
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output);

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output);

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
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output);

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
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
        State { installed: vec!["neovim".to_string()] }.save(&ctx.state_path()).unwrap();
        let mut input = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let result = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
            ).unwrap();
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
        run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output).unwrap();

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
            ).unwrap();
        }
        let state = State {
            installed: vec!["neovim".to_string(), "ripgrep".to_string(), "zed".to_string()],
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
        ).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Install, &mut input, &mut output).unwrap();

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
        ).unwrap();
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
        ).unwrap();
        let ripgrep_dir = ctx.packages_dir().join("ripgrep");
        std::fs::create_dir_all(&ripgrep_dir).unwrap();
        std::fs::write(
            ripgrep_dir.join(format!("install.{ext}")),
            "#!/usr/bin/env sh\necho 'RIPGREP_MARKER'\n",
        ).unwrap();
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
            ).unwrap();
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
        ).unwrap();

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
        ).unwrap();
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
        run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output).unwrap();

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
        let _ = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
        run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output).unwrap();

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
        run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output).unwrap();

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
        let state = State {
            installed: vec![],
        };
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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Update, &mut input, &mut output);

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
        let result = run_action(&ctx, &["neovim".to_string()], Action::Uninstall, &mut input, &mut output);

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
}
