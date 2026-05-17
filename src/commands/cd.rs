use crate::commands::detect_shell;
use crate::context::Context;
use std::path::PathBuf;
use std::process::Command;

/// Resolve and validate the target directory for `homeos cd`.
/// Returns the data directory path if it exists, or an error if not.
pub fn resolve_target(ctx: &Context) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = ctx.data_dir().to_path_buf();
    if !dir.exists() {
        return Err(format!(
            "Data directory not found at {}. Run 'homeos init' first.",
            dir.display()
        )
        .into());
    }
    Ok(dir)
}

pub fn run(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_target(ctx)?;
    let shell = detect_shell();

    let status = Command::new(&shell).current_dir(&dir).status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().join("homeos")));
        (tmp, ctx)
    }

    #[test]
    fn test_resolve_target_returns_data_dir() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx, None, false).unwrap();

        // Act
        let result = resolve_target(&ctx).unwrap();

        // Assert
        assert_eq!(result, ctx.data_dir());
    }

    #[test]
    fn test_resolve_target_dir_exists() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx, None, false).unwrap();

        // Act
        let result = resolve_target(&ctx).unwrap();

        // Assert
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn test_resolve_target_errors_when_not_initialized() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        let result = resolve_target(&ctx);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Data directory not found"));
        assert!(err.contains("homeos init"));
    }
}
