use crate::config::Config;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};

/// A plan describing which packages will be acted on and which are skipped.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub action: String,
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
    pub already_installed: Vec<String>,
}

impl Plan {
    /// Build a plan for the given action and package names.
    /// Looks up each package in the config to determine enabled/disabled status.
    /// If `installed` is provided, already-installed packages are classified separately
    /// and will not be executed.
    pub fn build(
        config: &Config,
        packages: &[String],
        action: &str,
        installed: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut enabled = Vec::new();
        let mut disabled = Vec::new();
        let mut already_installed = Vec::new();

        for name in packages {
            let pkg = config
                .packages
                .get(name)
                .ok_or_else(|| format!("Package '{name}' not found"))?;

            if !pkg.enabled {
                disabled.push(name.clone());
            } else if installed.contains(name) {
                already_installed.push(name.clone());
            } else {
                enabled.push(name.clone());
            }
        }

        Ok(Plan {
            action: action.to_string(),
            enabled,
            disabled,
            already_installed,
        })
    }

    /// Format the plan as a human-readable string for display.
    pub fn display(&self) -> String {
        let mut lines = Vec::new();

        let verb = match self.action.as_str() {
            "install" => "installed",
            "update" => "updated",
            "uninstall" => "uninstalled",
            other => other,
        };

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
pub fn confirm_plan<R: BufRead, W: Write>(
    plan: &Plan,
    reader: &mut R,
    writer: &mut W,
) -> bool {
    let display = plan.display();
    writeln!(writer, "{display}").ok();
    writeln!(writer).ok();
    prompt_confirm(reader, writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PackageConfig;
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
        let sut = Plan::build(&config, &packages, "install", &[]).unwrap();

        // Assert
        assert_eq!(sut.enabled, vec!["neovim", "zed"]);
        assert_eq!(sut.disabled, vec!["docker"]);
        assert_eq!(sut.action, "install");
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
        let sut = Plan::build(&config, &packages, "update", &[]).unwrap();

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
        let sut = Plan::build(&config, &packages, "install", &[]).unwrap();

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
        let result = Plan::build(&config, &packages, "install", &[]);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_display_shows_enabled_and_disabled() {
        // Arrange
        let plan = Plan {
            action: "install".to_string(),
            enabled: vec!["neovim".to_string(), "zed".to_string()],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
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
            action: "install".to_string(),
            enabled: vec![],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
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
            action: "update".to_string(),
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
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
            action: "uninstall".to_string(),
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
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
            action: "install".to_string(),
            enabled: vec!["neovim".to_string()],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
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
            action: "install".to_string(),
            enabled: vec![],
            disabled: vec!["docker".to_string()],
            already_installed: vec![],
        };

        // Act & Assert
        assert!(plan.is_empty());
    }

    #[test]
    fn test_is_not_empty_when_has_enabled_packages() {
        // Arrange
        let plan = Plan {
            action: "install".to_string(),
            enabled: vec!["neovim".to_string()],
            disabled: vec![],
            already_installed: vec![],
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
        let sut = Plan::build(&config, &packages, "install", &installed).unwrap();

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
        let sut = Plan::build(&config, &packages, "install", &installed).unwrap();

        // Assert
        assert!(sut.enabled.is_empty());
        assert!(sut.is_empty());
        assert_eq!(sut.already_installed, vec!["neovim", "zed"]);
    }

    #[test]
    fn test_display_shows_already_installed() {
        // Arrange
        let plan = Plan {
            action: "install".to_string(),
            enabled: vec!["zed".to_string()],
            disabled: vec![],
            already_installed: vec!["neovim".to_string()],
        };

        // Act
        let sut = plan.display();

        // Assert
        assert!(sut.contains("Skipping neovim (already installed)"));
        assert!(sut.contains("will be installed"));
        assert!(sut.contains("zed"));
    }
}
