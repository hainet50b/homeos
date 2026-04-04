mod action;
mod registry;

pub use registry::{list, add, add_dep, remove_dep, remove, enable, disable, cat};
pub use action::{install, update, uninstall};

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
        "pwsh"
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

    #[test]
    fn test_mod_only_contains_shared_helpers_and_reexports() {
        // Arrange — the module should re-export action functions from action.rs
        // Verify the re-exports are callable through the package module path

        // Act — confirm that the re-exported functions have the expected signatures
        // by taking function pointers (this fails at compile time if the signatures change)
        let _install_fn: fn(&crate::context::Context, &[String]) -> Result<(), Box<dyn std::error::Error>> = install;
        let _update_fn: fn(&crate::context::Context, &[String]) -> Result<(), Box<dyn std::error::Error>> = update;
        let _uninstall_fn: fn(&crate::context::Context, &[String], bool) -> Result<(), Box<dyn std::error::Error>> = uninstall;

        // Assert — if this compiles, install/update/uninstall are properly re-exported
        // from action.rs through mod.rs with correct signatures
    }
}
