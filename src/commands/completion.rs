use clap::ValueEnum;
use clap_complete::env::Shells;
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

impl CompletionShell {
    fn as_engine_name(self) -> &'static str {
        match self {
            CompletionShell::Bash => "bash",
            CompletionShell::Zsh => "zsh",
            CompletionShell::Fish => "fish",
            CompletionShell::PowerShell => "powershell",
            CompletionShell::Elvish => "elvish",
        }
    }
}

pub fn run(shell: CompletionShell) -> Result<(), Box<dyn std::error::Error>> {
    run_to(shell, &mut std::io::stdout())
}

fn run_to<W: Write>(
    shell: CompletionShell,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = shell.as_engine_name();
    let shells = Shells::builtins();
    let completer = shells
        .completer(name)
        .ok_or_else(|| format!("unknown shell `{name}`"))?;
    completer.write_registration("COMPLETE", "homeos", "homeos", "homeos", writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Commands};
    use clap::{CommandFactory, Parser};

    #[test]
    fn test_completion_bash_generates_registration_snippet() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Bash, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("_clap_complete_homeos"));
        assert!(output.contains("_CLAP_COMPLETE_INDEX"));
        assert!(output.contains("complete -o nospace"));
        assert!(output.contains("COMPLETE=\"bash\""));
    }

    #[test]
    fn test_completion_zsh_generates_registration_snippet() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Zsh, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("#compdef homeos"));
        assert!(output.contains("_clap_dynamic_completer_homeos"));
        assert!(output.contains("_CLAP_COMPLETE_INDEX"));
        assert!(output.contains("COMPLETE=\"zsh\""));
    }

    #[test]
    fn test_completion_fish_generates_registration_snippet() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Fish, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("complete --keep-order --exclusive --command homeos"));
        assert!(output.contains("COMPLETE=fish"));
    }

    #[test]
    fn test_completion_powershell_generates_registration_snippet() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::PowerShell, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Register-ArgumentCompleter -Native -CommandName homeos"));
        assert!(output.contains("Invoke-Expression"));
        assert!(output.contains("$env:COMPLETE"));
    }

    #[test]
    fn test_completion_elvish_generates_registration_snippet() {
        // Arrange
        let mut buf: Vec<u8> = Vec::new();

        // Act
        run_to(CompletionShell::Elvish, &mut buf).unwrap();

        // Assert
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("edit:completion:arg-completer[homeos]"));
        assert!(output.contains("_CLAP_COMPLETE_INDEX"));
        assert!(output.contains("COMPLETE=\"elvish\""));
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
