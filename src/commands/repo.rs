use crate::context::Context;
use crate::plan::prompt_confirm;
use crate::state::State;
use std::io::{BufRead, Write};
use std::process::Command;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_to(ctx, &mut std::io::stdout())
}

pub fn add(ctx: &Context, repo: &str, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repos_dir = ctx.repos_dir();
    let target = repos_dir.join(repo);

    if target.exists() {
        return Err(format!("Repository '{}' already exists", repo).into());
    }

    std::fs::create_dir_all(&repos_dir)?;

    let output = Command::new("git")
        .args(["clone", url, &target.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()).into());
    }

    println!("Repository '{}' cloned successfully", repo);
    Ok(())
}

pub fn remove(ctx: &Context, repo: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();
    remove_to(ctx, repo, &mut reader, &mut writer)
}

fn remove_to<R: BufRead, W: Write>(
    ctx: &Context,
    repo: &str,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    if repo == "default" {
        return Err("Cannot remove the default repository.".into());
    }

    let target = ctx.repos_dir().join(repo);

    if !target.exists() {
        return Err(format!("Repository '{}' does not exist", repo).into());
    }

    let state_path = target.join("state.yml");
    if state_path.exists() {
        let state = State::load(&state_path)?;
        if !state.installed.is_empty() {
            return Err(format!(
                "Repository '{}' has installed packages. Uninstall them first.",
                repo
            )
            .into());
        }
    }

    writeln!(writer, "Remove repository '{}'?", repo)?;
    if !prompt_confirm(reader, writer) {
        writeln!(writer, "Aborted.")?;
        return Ok(());
    }

    std::fs::remove_dir_all(&target)?;
    writeln!(writer, "Repository '{}' removed", repo)?;
    Ok(())
}

fn list_to<W: Write>(ctx: &Context, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let repos_dir = ctx.repos_dir();

    if !repos_dir.exists() {
        writeln!(writer, "No repositories.")?;
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

    if repos.is_empty() {
        writeln!(writer, "No repositories.")?;
        return Ok(());
    }

    for repo in &repos {
        writeln!(writer, "{repo}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn setup_context(base_dir: &TempDir) -> Context {
        Context::new(Some(base_dir.path().to_path_buf()), "default".to_string())
    }

    fn create_local_git_repo(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        Command::new("git")
            .args(["init", &dir.to_string_lossy()])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
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
        assert_eq!(String::from_utf8(output).unwrap(), "No repositories.\n");
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
        assert_eq!(String::from_utf8(output).unwrap(), "No repositories.\n");
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

    #[test]
    fn test_add_clones_repo() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let result = add(&ctx, "my-repo", &source_dir.path().to_string_lossy());

        // Assert
        assert!(result.is_ok());
        assert!(ctx.repos_dir().join("my-repo").exists());
        assert!(ctx.repos_dir().join("my-repo").join(".git").exists());
    }

    #[test]
    fn test_add_creates_repos_dir() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());
        assert!(!ctx.repos_dir().exists());

        // Act
        let result = add(&ctx, "new-repo", &source_dir.path().to_string_lossy());

        // Assert
        assert!(result.is_ok());
        assert!(ctx.repos_dir().join("new-repo").exists());
    }

    #[test]
    fn test_add_already_exists() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        std::fs::create_dir_all(ctx.repos_dir().join("existing")).unwrap();

        // Act
        let result = add(&ctx, "existing", "https://example.com/repo.git");

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Repository 'existing' already exists");
    }

    #[test]
    fn test_remove_rejects_default_repo() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        std::fs::create_dir_all(ctx.repos_dir().join("default")).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "default", &mut reader, &mut output);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Cannot remove the default repository.");
        assert!(ctx.repos_dir().join("default").exists());
    }

    #[test]
    fn test_remove_existing_repo_confirmed() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repo_dir = ctx.repos_dir().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("somefile.txt"), "data").unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "my-repo", &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(!repo_dir.exists());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Remove repository 'my-repo'?"));
        assert!(written.contains("Proceed? [y/N]"));
        assert!(written.contains("Repository 'my-repo' removed"));
    }

    #[test]
    fn test_remove_existing_repo_declined() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repo_dir = ctx.repos_dir().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("somefile.txt"), "data").unwrap();
        let mut reader = Cursor::new(b"n\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "my-repo", &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(repo_dir.exists());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("Aborted."));
        assert!(!written.contains("Repository 'my-repo' removed"));
    }

    #[test]
    fn test_remove_nonexistent_repo() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        std::fs::create_dir_all(ctx.repos_dir()).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "no-such-repo", &mut reader, &mut output);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Repository 'no-such-repo' does not exist");
    }

    #[test]
    fn test_remove_does_not_affect_other_repos() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repos_dir = ctx.repos_dir();
        std::fs::create_dir_all(repos_dir.join("repo-a")).unwrap();
        std::fs::create_dir_all(repos_dir.join("repo-b")).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "repo-a", &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(!repos_dir.join("repo-a").exists());
        assert!(repos_dir.join("repo-b").exists());
    }

    #[test]
    fn test_remove_rejects_repo_with_installed_packages() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repo_dir = ctx.repos_dir().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let state = crate::state::State {
            installed: vec!["neovim".to_string(), "zed".to_string()],
        };
        state.save(&repo_dir.join("state.yml")).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "my-repo", &mut reader, &mut output);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Repository 'my-repo' has installed packages. Uninstall them first."
        );
        assert!(repo_dir.exists());
    }

    #[test]
    fn test_remove_allows_repo_with_empty_state() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repo_dir = ctx.repos_dir().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let state = crate::state::State { installed: vec![] };
        state.save(&repo_dir.join("state.yml")).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "my-repo", &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(!repo_dir.exists());
    }

    #[test]
    fn test_remove_allows_repo_without_state_file() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);
        let repo_dir = ctx.repos_dir().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "my-repo", &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(!repo_dir.exists());
    }

    #[test]
    fn test_add_invalid_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = setup_context(&base_dir);

        // Act
        let result = add(&ctx, "bad-repo", "not-a-valid-url");

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.starts_with("git clone failed:"));
    }
}
