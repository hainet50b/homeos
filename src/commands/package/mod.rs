mod action;
mod registry;

pub use registry::{add, cat, disable, enable, list, remove};

use crate::plan::Action;
use crate::context::Context;
use crate::state::State;
use std::io::BufRead;
use std::io::Write;

/// Returns the OS-appropriate script file extension.
pub(crate) fn script_extension() -> &'static str {
    if cfg!(windows) {
        "ps1"
    } else {
        "sh"
    }
}

/// Returns the OS-appropriate shell command for executing scripts.
pub(crate) fn shell_command() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "sh"
    }
}

pub fn install(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    action::run_action(
        ctx,
        packages,
        Action::Install,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout(),
    )
}

pub fn update(ctx: &Context, packages: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    action::run_action(
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

    action::run_action(ctx, &resolved_packages, Action::Uninstall, reader, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_extension_returns_os_appropriate_value() {
        // Arrange — no setup needed, function depends only on compile-time cfg

        // Act
        let ext = script_extension();

        // Assert
        if cfg!(windows) {
            assert_eq!(ext, "ps1");
        } else {
            assert_eq!(ext, "sh");
        }
    }

    #[test]
    fn test_shell_command_returns_os_appropriate_value() {
        // Arrange — no setup needed, function depends only on compile-time cfg

        // Act
        let cmd = shell_command();

        // Assert
        if cfg!(windows) {
            assert_eq!(cmd, "powershell");
        } else {
            assert_eq!(cmd, "sh");
        }
    }
}
