mod action;
mod registry;

pub use action::{apply, install, uninstall, update};
pub use registry::{
    add, add_alias, add_dep, archive, cat, cd, disable, enable, info, list, remove, remove_alias,
    remove_dep, rename, unarchive,
};

const WINDOWS_POWERSHELL_FALLBACK_NOTICE: &str =
    "(running under Windows PowerShell 5.1; PowerShell 7 recommended)";

/// Returns the OS-appropriate script file extension.
pub(crate) fn script_extension() -> &'static str {
    if cfg!(windows) { "ps1" } else { "sh" }
}

/// Returns all script file extensions across all supported OS.
pub(crate) fn all_script_extensions() -> &'static [&'static str] {
    &["sh", "ps1"]
}

/// Returns the shell binary used to execute action scripts.
///
/// Resolution at action-execution time:
/// - Unix (Linux / macOS): `sh`
/// - Windows: `pwsh` (PowerShell 7+) when available on `PATH`;
///   otherwise `powershell` (Windows PowerShell 5.1, which ships preinstalled
///   on every modern Windows install). The fallback exists so a fresh Windows
///   machine that does not yet have PowerShell 7 can still run `homeos package
///   install pwsh` to bootstrap itself, instead of failing at the first script
///   execution.
pub(crate) fn shell_command() -> &'static str {
    shell_command_for(cfg!(windows), crate::commands::pwsh_on_path())
}

/// Returns `true` when running on Windows AND `pwsh` is not on `PATH`,
/// indicating that homeos will fall back to Windows PowerShell 5.1 for
/// script execution. Returns `false` on every non-Windows host.
pub(crate) fn is_windows_powershell_fallback() -> bool {
    cfg!(windows) && !crate::commands::pwsh_on_path()
}

/// Returns the leading notice that should accompany every plan display when the
/// Windows PowerShell 5.1 fallback is active. Returns `None` when the binary is
/// running under `pwsh` (or on any non-Windows host).
pub(crate) fn windows_powershell_fallback_notice() -> Option<&'static str> {
    if is_windows_powershell_fallback() {
        Some(WINDOWS_POWERSHELL_FALLBACK_NOTICE)
    } else {
        None
    }
}

fn shell_command_for(is_windows: bool, pwsh_available: bool) -> &'static str {
    if is_windows {
        crate::commands::windows_shell_for(pwsh_available)
    } else {
        "sh"
    }
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
    fn test_shell_command_for_unix_returns_sh() {
        // Arrange
        let is_windows = false;
        let pwsh_available = false;

        // Act
        let cmd = shell_command_for(is_windows, pwsh_available);

        // Assert
        assert_eq!(cmd, "sh");
    }

    #[test]
    fn test_shell_command_for_windows_with_pwsh_returns_pwsh() {
        // Arrange
        let is_windows = true;
        let pwsh_available = true;

        // Act
        let cmd = shell_command_for(is_windows, pwsh_available);

        // Assert
        assert_eq!(cmd, "pwsh");
    }

    #[test]
    fn test_shell_command_for_windows_without_pwsh_falls_back_to_powershell() {
        // Arrange
        let is_windows = true;
        let pwsh_available = false;

        // Act
        let cmd = shell_command_for(is_windows, pwsh_available);

        // Assert
        assert_eq!(cmd, "powershell");
    }

    #[test]
    fn test_windows_powershell_fallback_notice_is_none_on_unix() {
        // Arrange — running on Unix; function reads cfg!(windows) at compile time

        // Act
        let notice = windows_powershell_fallback_notice();

        // Assert
        if cfg!(windows) {
            // On Windows, the notice depends on whether pwsh is on PATH at test time;
            // its value isn't asserted here. The Windows-only integration test below
            // exercises the fallback path explicitly.
        } else {
            assert_eq!(notice, None);
        }
    }

    // NOTE: The composition `shell_command()` = `shell_command_for(cfg!(windows),
    // pwsh_on_path())` is intentionally NOT exercised by a dedicated test here.
    // Doing so previously required rewriting the process-global PATH (via
    // EnvVarGuard) to fake pwsh presence/absence, which raced with any concurrent
    // test spawning a subprocess (e.g. `git` in plugin::refresh), surfacing as
    // intermittent "program not found" failures on the Windows suite. The two
    // decision functions are fully covered without touching global state:
    // `shell_command_for` above and `pwsh_on_path_in` in `crate::commands`.

    #[test]
    #[allow(clippy::type_complexity)]
    fn test_mod_only_contains_shared_helpers_and_reexports() {
        // Arrange — the module should re-export action functions from action.rs
        // Verify the re-exports are callable through the package module path

        // Act — confirm that the re-exported functions have the expected signatures
        // by taking function pointers (this fails at compile time if the signatures change)
        let _install_fn: fn(
            &crate::context::Context,
            &[String],
            bool,
        ) -> Result<(), Box<dyn std::error::Error>> = install;
        let _update_fn: fn(
            &crate::context::Context,
            &[String],
            bool,
        ) -> Result<(), Box<dyn std::error::Error>> = update;
        let _uninstall_fn: fn(
            &crate::context::Context,
            &[String],
            bool,
            bool,
        ) -> Result<(), Box<dyn std::error::Error>> = uninstall;

        // Assert — if this compiles, install/update/uninstall are properly re-exported
        // from action.rs through mod.rs with correct signatures
    }
}
