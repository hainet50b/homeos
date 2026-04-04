use crate::config::Config;
use std::fmt;
use std::io::{BufRead, Write};

/// The three actions that can be performed on a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Uninstall,
}

impl Action {
    /// Past tense verb for plan display (e.g., "installed").
    pub fn past_tense(self) -> &'static str {
        match self {
            Action::Install => "installed",
            Action::Update => "updated",
            Action::Uninstall => "uninstalled",
        }
    }

    /// Present participle for progress messages (e.g., "Installing").
    pub fn gerund(self) -> &'static str {
        match self {
            Action::Install => "Installing",
            Action::Update => "Updating",
            Action::Uninstall => "Uninstalling",
        }
    }

    /// Action name as used in script filenames and overrides (e.g., "install").
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::Update => "update",
            Action::Uninstall => "uninstall",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A plan describing which packages will be acted on and which are skipped.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub action: Action,
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
    pub already_installed: Vec<String>,
    pub not_installed: Vec<String>,
}

impl Plan {
    /// Build a plan for the given action and package names.
    /// Looks up each package in the config to determine enabled/disabled status.
    /// If `installed` is provided, already-installed packages are classified separately
    /// and will not be executed.
    pub fn build(
        config: &Config,
        packages: &[String],
        action: Action,
        installed: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut enabled = Vec::new();
        let mut disabled = Vec::new();
        let mut already_installed = Vec::new();
        let mut not_installed = Vec::new();

        for name in packages {
            let pkg = config
                .packages
                .get(name)
                .ok_or_else(|| format!("Package '{name}' not found"))?;

            let in_state = installed.contains(name);

            match action {
                Action::Install => {
                    if !pkg.enabled {
                        disabled.push(name.clone());
                    } else if in_state {
                        already_installed.push(name.clone());
                    } else {
                        enabled.push(name.clone());
                    }
                }
                Action::Update => {
                    if !pkg.enabled {
                        disabled.push(name.clone());
                    } else if in_state {
                        enabled.push(name.clone());
                    } else {
                        not_installed.push(name.clone());
                    }
                }
                Action::Uninstall => {
                    if in_state {
                        enabled.push(name.clone());
                    } else {
                        not_installed.push(name.clone());
                    }
                }
            }
        }

        Ok(Plan {
            action,
            enabled,
            disabled,
            already_installed,
            not_installed,
        })
    }

    /// Format the plan as a human-readable string for display.
    pub fn display(&self) -> String {
        let mut lines = Vec::new();

        let verb = self.action.past_tense();

        if !self.enabled.is_empty() {
            lines.push(format!("The following packages will be {verb}:"));
            for name in &self.enabled {
                lines.push(format!("  {name}"));
            }
        }

        for name in &self.disabled {
            lines.push(format!("Skipping {name} (disabled)"));
        }

        for name in &self.already_installed {
            lines.push(format!("Skipping {name} (already installed)"));
        }

        for name in &self.not_installed {
            lines.push(format!("Skipping {name} (not installed)"));
        }

        lines.join("\n")
    }

    /// Returns true if there are no enabled packages to act on.
    pub fn is_empty(&self) -> bool {
        self.enabled.is_empty()
    }
}

/// Prompt the user for confirmation, reading from the provided reader.
/// Returns true only if the user enters "y" or "Y".
pub fn prompt_confirm<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> bool {
    write!(writer, "Proceed? [y/N] ").ok();
    writer.flush().ok();

    let mut input = String::new();
    if reader.read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim(), "y" | "Y")
}

/// Show the plan and prompt for confirmation. Returns true if the user confirms.
pub fn confirm_plan<R: BufRead, W: Write>(plan: &Plan, reader: &mut R, writer: &mut W) -> bool {
    let display = plan.display();
    writeln!(writer, "{display}").ok();
    writeln!(writer).ok();
    prompt_confirm(reader, writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PackageConfig;
    use std::collections::BTreeMap;
    use std::io::Cursor;

    fn fixture_config(packages: Vec<(&str, bool)>) -> Config {
        let mut map = BTreeMap::new();
        for (name, enabled) in packages {
            map.insert(
                name.to_string(),
                PackageConfig {
                    enabled,
                    ..Default::default()
                },
            );
        }
        Config { packages: map }
    }

    #[test]
    fn test_build_plan_separates_enabled_and_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", true), ("docker", false), ("zed", true)]);
        let packages: Vec<String> = vec!["neovim", "docker", "zed"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim", "zed"]);
        assert_eq!(sut.disabled, vec!["docker"]);
        assert_eq!(sut.action, Action::Install);
    }

    #[test]
    fn test_build_plan_all_enabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", true), ("ripgrep", true)]);
        let packages: Vec<String> = vec!["neovim", "ripgrep"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim", "ripgrep"]);
        assert!(sut.disabled.is_empty());
    }

    #[test]
    fn test_build_plan_all_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false), ("docker", false)]);
        let packages: Vec<String> = vec!["neovim", "docker"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.disabled, vec!["neovim", "docker"]);
        assert!(sut.is_empty());
    }

    #[test]
    fn test_build_plan_errors_on_unknown_package() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["nonexistent"].into_iter().map(String::from).collect();

        // Act
        let result = Plan::build(&config, &packages, Action::Install, &[]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_display_shows_enabled_and_disabled() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec!["neovim".to_string(), "zed".to_string()],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act
        let sut = plan.display();

        // Assert
        let expected = "\
The following packages will be installed:
  neovim
  zed
Skipping docker (disabled)";
        assert_eq!(sut, expected);
    }

    #[test]
    fn test_display_only_disabled() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec![],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert_eq!(sut, "Skipping docker (disabled)");
    }

    #[test]
    fn test_display_update_verb() {
        // Arrange
        let plan = Plan {
            action: Action::Update,
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert!(sut.contains("will be updated"));
    }

    #[test]
    fn test_display_uninstall_verb() {
        // Arrange
        let plan = Plan {
            action: Action::Uninstall,
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert!(sut.contains("will be uninstalled"));
    }

    #[test]
    fn test_prompt_confirm_accepts_y() {
        // Arrange
        let mut input = Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let sut = prompt_confirm(&mut input, &mut output);

        // Assert
        assert!(sut);
    }

    #[test]
    fn test_prompt_confirm_accepts_uppercase_y() {
        // Arrange
        let mut input = Cursor::new(b"Y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let sut = prompt_confirm(&mut input, &mut output);

        // Assert
        assert!(sut);
    }

    #[test]
    fn test_prompt_confirm_rejects_n() {
        // Arrange
        let mut input = Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        let sut = prompt_confirm(&mut input, &mut output);

        // Assert
        assert!(!sut);
    }

    #[test]
    fn test_prompt_confirm_rejects_empty() {
        // Arrange
        let mut input = Cursor::new(b"\n".to_vec());
        let mut output = Vec::new();

        // Act
        let sut = prompt_confirm(&mut input, &mut output);

        // Assert
        assert!(!sut);
    }

    #[test]
    fn test_prompt_confirm_writes_prompt_text() {
        // Arrange
        let mut input = Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();

        // Act
        prompt_confirm(&mut input, &mut output);

        // Assert
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Proceed? [y/N]"));
    }

    #[test]
    fn test_confirm_plan_shows_plan_and_prompts() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec!["neovim".to_string()],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
            not_installed: vec![],
        };
        let mut input = Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();

        // Act
        let sut = confirm_plan(&plan, &mut input, &mut output);

        // Assert
        assert!(sut);
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("neovim"));
        assert!(written.contains("Skipping docker (disabled)"));
        assert!(written.contains("Proceed? [y/N]"));
    }

    #[test]
    fn test_is_empty_when_no_enabled_packages() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec![],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act & Assert
        assert!(plan.is_empty());
    }

    #[test]
    fn test_is_not_empty_when_has_enabled_packages() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
            not_installed: vec![],
        };

        // Act & Assert
        assert!(!plan.is_empty());
    }

    #[test]
    fn test_build_plan_classifies_already_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", true), ("zed", true), ("docker", false)]);
        let packages: Vec<String> = vec!["neovim", "zed", "docker"]
            .into_iter()
            .map(String::from)
            .collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &installed).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["zed"]);
        assert_eq!(sut.disabled, vec!["docker"]);
        assert_eq!(sut.already_installed, vec!["neovim"]);
    }

    #[test]
    fn test_build_plan_all_already_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", true), ("zed", true)]);
        let packages: Vec<String> = vec!["neovim", "zed"]
            .into_iter()
            .map(String::from)
            .collect();
        let installed = vec!["neovim".to_string(), "zed".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &installed).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert!(sut.is_empty());
        assert_eq!(sut.already_installed, vec!["neovim", "zed"]);
    }

    #[test]
    fn test_display_shows_already_installed() {
        // Arrange
        let plan = Plan {
            action: Action::Install,
            enabled: vec!["zed".to_string()],
            disabled: vec![],
            already_installed: vec!["neovim".to_string()],
            not_installed: vec![],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert!(sut.contains("Skipping neovim (already installed)"));
        assert!(sut.contains("will be installed"));
        assert!(sut.contains("zed"));
    }

    #[test]
    fn test_build_plan_uninstall_ignores_disabled_status() {
        // Arrange
        let config = fixture_config(vec![("neovim", false), ("zed", true)]);
        let packages: Vec<String> = vec!["neovim", "zed"]
            .into_iter()
            .map(String::from)
            .collect();
        let installed = vec!["neovim".to_string(), "zed".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Uninstall, &installed).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim", "zed"]);
        assert!(sut.disabled.is_empty());
    }

    #[test]
    fn test_build_plan_install_still_skips_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false), ("zed", true)]);
        let packages: Vec<String> = vec!["neovim", "zed"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["zed"]);
        assert_eq!(sut.disabled, vec!["neovim"]);
    }

    #[test]
    fn test_build_plan_has_empty_not_installed_by_default() {
        // Arrange
        let config = fixture_config(vec![("neovim", true), ("zed", true)]);
        let packages: Vec<String> = vec!["neovim", "zed"]
            .into_iter()
            .map(String::from)
            .collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert!(sut.not_installed.is_empty());
    }

    #[test]
    fn test_build_plan_already_installed_only_applies_to_install_action() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Update, &installed).unwrap();

        // Assert — for update, being in state means execute (enabled), not already_installed
        assert_eq!(sut.enabled, vec!["neovim"]);
        assert!(sut.already_installed.is_empty());
    }

    #[test]
    fn test_display_shows_not_installed() {
        // Arrange
        let plan = Plan {
            action: Action::Update,
            enabled: vec!["zed".to_string()],
            disabled: vec![],
            already_installed: vec![],
            not_installed: vec!["neovim".to_string()],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert!(sut.contains("Skipping neovim (not installed)"));
        assert!(sut.contains("will be updated"));
        assert!(sut.contains("zed"));
    }

    // --- Behavior matrix tests ---

    #[test]
    fn test_behavior_matrix_install_enabled_not_in_state_executes() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_install_enabled_in_state_skips_already_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &installed).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.already_installed, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_install_disabled_not_in_state_skips_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.disabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_install_disabled_in_state_skips_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Install, &installed).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.disabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_update_enabled_not_in_state_skips_not_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Update, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.not_installed, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_update_enabled_in_state_executes() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Update, &installed).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_update_disabled_not_in_state_skips_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Update, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.disabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_update_disabled_in_state_skips_disabled() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Update, &installed).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.disabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_uninstall_enabled_not_in_state_skips_not_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Uninstall, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.not_installed, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_uninstall_enabled_in_state_executes() {
        // Arrange
        let config = fixture_config(vec![("neovim", true)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Uninstall, &installed).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim"]);
    }

    #[test]
    fn test_behavior_matrix_uninstall_disabled_not_in_state_skips_not_installed() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();

        // Act
        let sut = Plan::build(&config, &packages, Action::Uninstall, &[]).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert_eq!(sut.not_installed, vec!["neovim"]);
        assert!(sut.disabled.is_empty());
    }

    #[test]
    fn test_behavior_matrix_uninstall_disabled_in_state_executes() {
        // Arrange
        let config = fixture_config(vec![("neovim", false)]);
        let packages: Vec<String> = vec!["neovim"].into_iter().map(String::from).collect();
        let installed = vec!["neovim".to_string()];

        // Act
        let sut = Plan::build(&config, &packages, Action::Uninstall, &installed).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim"]);
        assert!(sut.disabled.is_empty());
    }

    // --- Action enum tests ---

    #[test]
    fn test_action_as_str() {
        // Arrange
        let actions = [Action::Install, Action::Update, Action::Uninstall];

        // Act
        let names: Vec<&str> = actions.iter().map(|a| a.as_str()).collect();

        // Assert
        assert_eq!(names, vec!["install", "update", "uninstall"]);
    }

    #[test]
    fn test_action_past_tense() {
        // Arrange
        let actions = [Action::Install, Action::Update, Action::Uninstall];

        // Act
        let verbs: Vec<&str> = actions.iter().map(|a| a.past_tense()).collect();

        // Assert
        assert_eq!(verbs, vec!["installed", "updated", "uninstalled"]);
    }

    #[test]
    fn test_action_gerund() {
        // Arrange
        let actions = [Action::Install, Action::Update, Action::Uninstall];

        // Act
        let gerunds: Vec<&str> = actions.iter().map(|a| a.gerund()).collect();

        // Assert
        assert_eq!(gerunds, vec!["Installing", "Updating", "Uninstalling"]);
    }

    #[test]
    fn test_action_display() {
        // Arrange / Act / Assert
        assert_eq!(format!("{}", Action::Install), "install");
        assert_eq!(format!("{}", Action::Update), "update");
        assert_eq!(format!("{}", Action::Uninstall), "uninstall");
    }

    #[test]
    fn test_action_equality() {
        // Arrange / Act / Assert
        assert_eq!(Action::Install, Action::Install);
        assert_ne!(Action::Install, Action::Update);
        assert_ne!(Action::Update, Action::Uninstall);
    }
}
