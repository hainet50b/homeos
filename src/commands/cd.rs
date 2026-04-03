use crate::context::Context;
use std::path::PathBuf;
use std::process::Command;

/// Resolve and validate the target directory for `homeos cd`.
/// Returns the path if it exists, or an error if not.
pub fn resolve_target(ctx: &Context) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = ctx.default_repo_dir();
    if !dir.exists() {
        return Err(format!(
            "Default repository not found at {}. Run `homeos init` first.",
            dir.display()
        )
        .into());
    }
    Ok(dir)
}

pub fn run(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_target(ctx)?;
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

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
        let ctx = Context::new(Some(tmp.path().to_path_buf()));
        (tmp, ctx)
    }

    #[test]
    fn test_resolve_target_returns_default_repo_dir() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx).unwrap();

        // Act
        let result = resolve_target(&ctx).unwrap();

        // Assert
        assert_eq!(result, ctx.default_repo_dir());
    }

    #[test]
    fn test_resolve_target_dir_exists() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx).unwrap();

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
        assert!(err.contains("Default repository not found"));
        assert!(err.contains("homeos init"));
    }
}
