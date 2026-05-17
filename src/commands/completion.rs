use clap::{CommandFactory, ValueEnum};
use clap_complete::{Shell, generate};
use std::io::Write;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

pub fn run(shell: CompletionShell) -> Result<(), Box<dyn std::error::Error>> {
    run_to(shell, &mut std::io::stdout())
}

fn run_to<W: Write>(
    shell: CompletionShell,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = crate::Cli::command();
    let name = cmd.get_name().to_string();
    match shell {
        CompletionShell::Bash => generate(Shell::Bash, &mut cmd, name, writer),
        CompletionShell::Zsh => generate(Shell::Zsh, &mut cmd, name, writer),
        CompletionShell::Fish => generate(Shell::Fish, &mut cmd, name, writer),
        CompletionShell::PowerShell => generate(Shell::PowerShell, &mut cmd, name, writer),
        CompletionShell::Elvish => generate(Shell::Elvish, &mut cmd, name, writer),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn test_completion_bash_generates_script() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Bash, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("homeos"));
        assert!(output.contains("complete"));
    }

    #[test]
    fn test_completion_zsh_generates_script() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Zsh, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("homeos"));
        assert!(output.contains("#compdef"));
    }

    #[test]
    fn test_completion_fish_generates_script() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Fish, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("homeos"));
        assert!(output.contains("complete -c homeos"));
    }

    #[test]
    fn test_completion_powershell_generates_script() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::PowerShell, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("homeos"));
        assert!(output.contains("Register-ArgumentCompleter"));
    }

    #[test]
    fn test_completion_elvish_generates_script() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Elvish, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("homeos"));
        assert!(output.contains("edit:completion:arg-completer"));
    }

    #[test]
    fn test_completion_parses_lowercase_shell_names() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "completion", "bash"]).unwrap();

        // Assert
        if let Commands::Completion { shell } = cli.command {
            assert_eq!(shell, CompletionShell::Bash);
        } else {
            panic!("Expected Commands::Completion");
        }
    }

    #[test]
    fn test_completion_parses_powershell_as_lowercase() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "completion", "powershell"]).unwrap();

        // Assert
        if let Commands::Completion { shell } = cli.command {
            assert_eq!(shell, CompletionShell::PowerShell);
        } else {
            panic!("Expected Commands::Completion");
        }
    }

    #[test]
    fn test_completion_rejects_unknown_shell() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "completion", "tcsh"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected parse error"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_completion_help_lists_all_supported_shells() {
        // Arrange
        let cmd = Cli::command();
        let completion_cmd = cmd.find_subcommand("completion").unwrap();

        // Act
        let shell_arg = completion_cmd
            .get_positionals()
            .find(|a| a.get_id() == "shell")
            .unwrap();
        let values: Vec<String> = shell_arg
            .get_possible_values()
            .iter()
            .map(|v| v.get_name().to_string())
            .collect();

        // Assert
        assert_eq!(values, vec!["bash", "zsh", "fish", "powershell", "elvish"]);
    }
}
