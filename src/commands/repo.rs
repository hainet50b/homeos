use crate::context::Context;
use std::io::Write;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_to(ctx, &mut std::io::stdout())
}

fn list_to<W: Write>(ctx: &Context, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let repos_dir = ctx.repos_dir();

    if !repos_dir.exists() {
        return Ok(());
    }

    let mut repos: Vec<String> = std::fs::read_dir(&repos_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    repos.sort();

    for repo in &repos {
        writeln!(writer, "{repo}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_context(base_dir: &TempDir) -> Context {
        Context::new(Some(base_dir.path().to_path_buf()), "default".to_string())
    }

    #[test]
    fn test_list_no_repos_dir() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(String::from_utf8(output).unwrap(), "");
    }

    #[test]
    fn test_list_empty_repos_dir() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        std::fs::create_dir_all(ctx.repos_dir()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(String::from_utf8(output).unwrap(), "");
    }

    #[test]
    fn test_list_single_repo() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        std::fs::create_dir_all(ctx.repos_dir().join("default")).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(String::from_utf8(output).unwrap(), "default\n");
    }

    #[test]
    fn test_list_multiple_repos_sorted() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repos_dir = ctx.repos_dir();
        std::fs::create_dir_all(repos_dir.join("work")).unwrap();
        std::fs::create_dir_all(repos_dir.join("default")).unwrap();
        std::fs::create_dir_all(repos_dir.join("server")).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "default\nserver\nwork\n"
        );
    }

    #[test]
    fn test_list_ignores_files() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repos_dir = ctx.repos_dir();
        std::fs::create_dir_all(repos_dir.join("default")).unwrap();
        std::fs::write(repos_dir.join("some-file.txt"), "").unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        assert_eq!(String::from_utf8(output).unwrap(), "default\n");
    }
}
