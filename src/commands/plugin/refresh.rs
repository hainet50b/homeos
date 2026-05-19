use crate::config::Config;
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::git;
use crate::output::OutputFormat;
use crate::plan::prompt_confirm;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq)]
struct DiffSummary {
    modified: Vec<String>,
    added: Vec<String>,
    removed: Vec<String>,
}

impl DiffSummary {
    fn is_empty(&self) -> bool {
        self.modified.is_empty() && self.added.is_empty() && self.removed.is_empty()
    }
}

struct Target {
    name: String,
    url: Option<String>,
}

enum Outcome {
    LocalSkipped,
    UpToDate,
    Pending { diff: DiffSummary, staging: PathBuf },
}

pub fn run(
    ctx: &Context,
    plugin: Option<&str>,
    all: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();
    run_to(ctx, plugin, all, dry_run, &mut reader, &mut writer)
}

fn run_to<R: BufRead, W: Write>(
    ctx: &Context,
    plugin: Option<&str>,
    all: bool,
    dry_run: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    if plugin.is_none() && !all {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            "specify a plugin name or use --all",
        )
        .into());
    }

    let config = Config::load(&ctx.config_path())?;
    let format = ctx.output_format();

    let targets: Vec<Target> = if all {
        config
            .plugins
            .iter()
            .map(|(name, cfg)| Target {
                name: name.clone(),
                url: cfg.url.clone(),
            })
            .collect()
    } else {
        let name = plugin.unwrap();
        let cfg = config.plugins.get(name).ok_or_else(|| {
            HomeosError::new(
                reasons::PLUGIN_NOT_FOUND,
                format!("Plugin '{name}' not found"),
            )
        })?;
        vec![Target {
            name: name.to_string(),
            url: cfg.url.clone(),
        }]
    };

    if all && targets.is_empty() {
        if format == OutputFormat::Text {
            writeln!(writer, "No plugins to refresh")?;
        }
        return Ok(());
    }

    let mut results: Vec<(Target, Outcome)> = Vec::new();
    for target in targets {
        let outcome = process_one(ctx, &target, writer, format)?;
        let dry_status = match &outcome {
            Outcome::LocalSkipped => Some("local-skipped"),
            Outcome::UpToDate => Some("up-to-date"),
            Outcome::Pending { .. } => Some("would-refresh"),
        };
        if format == OutputFormat::Json
            && (dry_run || matches!(outcome, Outcome::LocalSkipped | Outcome::UpToDate))
        {
            emit_json_status(writer, &target, &outcome, dry_status.unwrap())?;
        }
        results.push((target, outcome));
    }

    if dry_run {
        cleanup_pending(&results)?;
        return Ok(());
    }

    let any_pending = results
        .iter()
        .any(|(_, o)| matches!(o, Outcome::Pending { .. }));

    let proceed = if !any_pending {
        false
    } else if ctx.yes() {
        true
    } else {
        if all {
            emit_aggregate_prompt(writer, &results)?;
        } else {
            writeln!(
                writer,
                "Local changes will be replaced with upstream content."
            )?;
        }
        prompt_confirm(reader, writer)
    };

    if !proceed {
        if any_pending && format == OutputFormat::Text {
            writeln!(writer, "Aborted.")?;
        }
        cleanup_pending(&results)?;
        return Ok(());
    }

    for (target, outcome) in &results {
        if let Outcome::Pending { staging, .. } = outcome {
            let final_target = ctx.plugins_dir().join(&target.name);
            if final_target.exists() {
                std::fs::remove_dir_all(&final_target)?;
            }
            std::fs::rename(staging, &final_target)?;
            match format {
                OutputFormat::Text => writeln!(writer, "Refreshed plugin '{}'", target.name)?,
                OutputFormat::Json => emit_json_status(writer, target, outcome, "refreshed")?,
            }
        }
    }

    Ok(())
}

fn process_one<W: Write>(
    ctx: &Context,
    target: &Target,
    writer: &mut W,
    format: OutputFormat,
) -> Result<Outcome, Box<dyn std::error::Error>> {
    let url = match &target.url {
        Some(u) => u.clone(),
        None => {
            if format == OutputFormat::Text {
                writeln!(
                    writer,
                    "Plugin '{}' is local; nothing to refresh",
                    target.name
                )?;
            }
            return Ok(Outcome::LocalSkipped);
        }
    };

    if format == OutputFormat::Text {
        writeln!(writer, "Fetching {} from {}...", target.name, url)?;
    }

    let plugins_dir = ctx.plugins_dir();
    std::fs::create_dir_all(&plugins_dir)?;
    let staging = plugins_dir.join(format!(".homeos-refresh-{}", target.name));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }

    git::clone(&url, &staging)?;

    if !staging.join("plugin.yml").exists() {
        std::fs::remove_dir_all(&staging)?;
        return Err(HomeosError::new(
            reasons::NOT_A_VALID_HOMEOS_PLUGIN,
            "Not a valid homeos plugin. Cloned directory removed.",
        )
        .into());
    }

    let git_dir = staging.join(".git");
    if git_dir.exists() {
        std::fs::remove_dir_all(&git_dir)?;
    }

    let current = plugins_dir.join(&target.name);
    let diff = compute_diff(&current, &staging)?;

    if diff.is_empty() {
        if format == OutputFormat::Text {
            writeln!(writer, "Plugin '{}' is up to date", target.name)?;
        }
        std::fs::remove_dir_all(&staging)?;
        return Ok(Outcome::UpToDate);
    }

    if format == OutputFormat::Text {
        writeln!(writer, "The following files differ from upstream:")?;
        for path in &diff.modified {
            writeln!(writer, "  M {path}")?;
        }
        for path in &diff.added {
            writeln!(writer, "  A {path}")?;
        }
        for path in &diff.removed {
            writeln!(writer, "  D {path}")?;
        }
    }

    Ok(Outcome::Pending { diff, staging })
}

fn collect_files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn std::error::Error>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }
    collect_files_rec(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_rec(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == ".git" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_files_rec(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root)?.to_path_buf();
            let content = std::fs::read(&path)?;
            out.insert(rel, content);
        }
    }
    Ok(())
}

fn compute_diff(old: &Path, new: &Path) -> Result<DiffSummary, Box<dyn std::error::Error>> {
    let old_files = collect_files(old)?;
    let new_files = collect_files(new)?;
    let mut diff = DiffSummary::default();
    let all_paths: BTreeSet<&PathBuf> = old_files.keys().chain(new_files.keys()).collect();
    for path in all_paths {
        let path_str = path.to_string_lossy().to_string();
        match (old_files.get(path), new_files.get(path)) {
            (None, Some(_)) => diff.added.push(path_str),
            (Some(_), None) => diff.removed.push(path_str),
            (Some(o), Some(n)) if o != n => diff.modified.push(path_str),
            _ => {}
        }
    }
    Ok(diff)
}

fn emit_aggregate_prompt<W: Write>(
    writer: &mut W,
    results: &[(Target, Outcome)],
) -> std::io::Result<()> {
    writeln!(writer, "Changes detected in:")?;
    for (target, outcome) in results {
        if let Outcome::Pending { diff, .. } = outcome {
            writeln!(
                writer,
                "  {} — {} modified, {} added, {} removed",
                target.name,
                diff.modified.len(),
                diff.added.len(),
                diff.removed.len(),
            )?;
        }
    }
    write!(writer, "Apply all? [y/N] ")?;
    writer.flush()
}

fn emit_json_status<W: Write>(
    writer: &mut W,
    target: &Target,
    outcome: &Outcome,
    status: &str,
) -> std::io::Result<()> {
    let url_value = match &target.url {
        Some(u) => serde_json::Value::String(u.clone()),
        None => serde_json::Value::Null,
    };
    let mut obj = serde_json::Map::new();
    obj.insert(
        "name".to_string(),
        serde_json::Value::String(target.name.clone()),
    );
    obj.insert("url".to_string(), url_value);
    obj.insert(
        "status".to_string(),
        serde_json::Value::String(status.to_string()),
    );
    if let Outcome::Pending { diff, .. } = outcome {
        let changes = serde_json::json!({
            "modified": diff.modified,
            "added": diff.added,
            "removed": diff.removed,
        });
        obj.insert("changes".to_string(), changes);
    }
    writeln!(writer, "{}", serde_json::Value::Object(obj))
}

fn cleanup_pending(results: &[(Target, Outcome)]) -> Result<(), Box<dyn std::error::Error>> {
    for (_, outcome) in results {
        if let Outcome::Pending { staging, .. } = outcome {
            if staging.exists() {
                std::fs::remove_dir_all(staging)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use std::io::Cursor;
    use std::process::Command;
    use tempfile::TempDir;

    fn fixture(base_dir: &TempDir) -> Context {
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
        std::fs::create_dir_all(ctx.data_dir()).unwrap();
        ctx
    }

    fn fixture_with_config(base_dir: &TempDir) -> Context {
        let ctx = fixture(base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        ctx
    }

    fn create_upstream_plugin(dir: &Path, plugin_yml: &str, install_tmpl: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("plugin.yml"), plugin_yml).unwrap();
        std::fs::write(dir.join("install.sh.tmpl"), install_tmpl).unwrap();
        Command::new("git")
            .args(["init", "--initial-branch=main", &dir.to_string_lossy()])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test",
                "add",
                "-A",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test",
                "commit",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
    }

    fn register_plugin(ctx: &Context, name: &str, url: Option<&str>) {
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            name.to_string(),
            PluginConfig {
                url: url.map(|s| s.to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
    }

    #[test]
    fn test_run_to_errors_when_no_plugin_and_no_all() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, None, false, false, &mut reader, &mut writer);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "specify a plugin name or use --all");
    }

    #[test]
    fn test_run_to_errors_when_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("nope"), false, false, &mut reader, &mut writer);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin 'nope' not found");
    }

    #[test]
    fn test_run_to_skips_local_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        register_plugin(&ctx, "custom", None);
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("custom"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Plugin 'custom' is local; nothing to refresh"));
    }

    #[test]
    fn test_run_to_reports_up_to_date_when_no_changes() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        // Mirror the upstream content into the local plugin dir.
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(
            local.join("plugin.yml"),
            "description: Test plugin\nparams: []\n",
        )
        .unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install\n").unwrap();
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Plugin 'dnf' is up to date"));
        assert!(!text.contains("Aborted."));
        // Staging dir must be cleaned up.
        assert!(!ctx.plugins_dir().join(".homeos-refresh-dnf").exists());
    }

    #[test]
    fn test_run_to_refreshes_modified_files_after_confirmation() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(
            local.join("plugin.yml"),
            "description: Test plugin\nparams: []\n",
        )
        .unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Fetching dnf from"));
        assert!(text.contains("The following files differ from upstream:"));
        assert!(text.contains("M install.sh.tmpl"));
        assert!(text.contains("Refreshed plugin 'dnf'"));
        let new_content = std::fs::read_to_string(local.join("install.sh.tmpl")).unwrap();
        assert_eq!(new_content, "sudo install NEW\n");
        assert!(!ctx.plugins_dir().join(".homeos-refresh-dnf").exists());
    }

    #[test]
    fn test_run_to_dry_run_does_not_replace_files() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, true, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("The following files differ from upstream:"));
        assert!(!text.contains("Refreshed plugin"));
        assert!(!text.contains("Apply"));
        let unchanged = std::fs::read_to_string(local.join("install.sh.tmpl")).unwrap();
        assert_eq!(unchanged, "sudo install OLD\n");
        assert!(!ctx.plugins_dir().join(".homeos-refresh-dnf").exists());
    }

    #[test]
    fn test_run_to_decline_keeps_files_and_prints_aborted() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"n\n");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Aborted."));
        let unchanged = std::fs::read_to_string(local.join("install.sh.tmpl")).unwrap();
        assert_eq!(unchanged, "sudo install OLD\n");
        assert!(!ctx.plugins_dir().join(".homeos-refresh-dnf").exists());
    }

    #[test]
    fn test_run_to_all_with_no_plugins_prints_no_plugins_to_refresh() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, None, true, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("No plugins to refresh"));
    }

    #[test]
    fn test_run_to_all_emits_aggregate_prompt() {
        // Arrange — two plugins, one with changes, one local
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        register_plugin(&ctx, "custom", None);
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"y\n");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, None, true, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Plugin 'custom' is local; nothing to refresh"));
        assert!(text.contains("Changes detected in:"));
        assert!(text.contains("dnf — 1 modified, 1 added, 0 removed"));
        assert!(text.contains("Apply all?"));
        assert!(text.contains("Refreshed plugin 'dnf'"));
    }

    #[test]
    fn test_run_to_yes_flag_skips_prompt() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir).with_yes(true);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        assert!(text.contains("Refreshed plugin 'dnf'"));
        assert!(!text.contains("Proceed?"));
    }

    #[test]
    fn test_run_to_invalid_cloned_plugin_returns_error() {
        // Arrange — upstream "repo" without plugin.yml
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let upstream = TempDir::new().unwrap();
        // Create a git repo but no plugin.yml
        std::fs::create_dir_all(upstream.path()).unwrap();
        std::fs::write(upstream.path().join("README.md"), "hi\n").unwrap();
        Command::new("git")
            .args([
                "init",
                "--initial-branch=main",
                &upstream.path().to_string_lossy(),
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &upstream.path().to_string_lossy(),
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test",
                "add",
                "-A",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &upstream.path().to_string_lossy(),
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test",
                "commit",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Not a valid homeos plugin. Cloned directory removed."
        );
        assert!(!ctx.plugins_dir().join(".homeos-refresh-dnf").exists());
    }

    #[test]
    fn test_run_to_json_dry_run_emits_ndjson_for_each_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir).with_output_format(OutputFormat::Json);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        register_plugin(&ctx, "custom", None);
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, None, true, true, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let custom: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(custom["name"], "custom");
        assert_eq!(custom["status"], "local-skipped");
        assert!(custom["url"].is_null());
        let dnf: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(dnf["name"], "dnf");
        assert_eq!(dnf["status"], "would-refresh");
        assert_eq!(dnf["changes"]["modified"][0], "install.sh.tmpl");
    }

    #[test]
    fn test_run_to_json_yes_emits_refreshed_status() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir)
            .with_output_format(OutputFormat::Json)
            .with_yes(true);
        let upstream = TempDir::new().unwrap();
        create_upstream_plugin(
            upstream.path(),
            "description: Test plugin\nparams: []\n",
            "sudo install NEW\n",
        );
        let url = upstream.path().to_string_lossy().to_string();
        register_plugin(&ctx, "dnf", Some(&url));
        let local = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("install.sh.tmpl"), "sudo install OLD\n").unwrap();
        let mut reader = Cursor::new(b"");
        let mut writer = Vec::new();

        // Act
        let result = run_to(&ctx, Some("dnf"), false, false, &mut reader, &mut writer);

        // Assert
        assert!(result.is_ok());
        let text = String::from_utf8(writer).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(value["name"], "dnf");
        assert_eq!(value["status"], "refreshed");
    }

    #[test]
    fn test_compute_diff_detects_added_modified_removed() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let old = tmp.path().join("old");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("same.txt"), "same").unwrap();
        std::fs::write(new.join("same.txt"), "same").unwrap();
        std::fs::write(old.join("modified.txt"), "old").unwrap();
        std::fs::write(new.join("modified.txt"), "new").unwrap();
        std::fs::write(old.join("removed.txt"), "bye").unwrap();
        std::fs::write(new.join("added.txt"), "hello").unwrap();

        // Act
        let diff = compute_diff(&old, &new).unwrap();

        // Assert
        assert_eq!(diff.modified, vec!["modified.txt".to_string()]);
        assert_eq!(diff.added, vec!["added.txt".to_string()]);
        assert_eq!(diff.removed, vec!["removed.txt".to_string()]);
    }

    #[test]
    fn test_compute_diff_excludes_git_directory() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let old = tmp.path().join("old");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(new.join(".git")).unwrap();
        std::fs::write(new.join(".git/HEAD"), "ref: ...").unwrap();
        std::fs::write(new.join("plugin.yml"), "params: []\n").unwrap();
        std::fs::write(old.join("plugin.yml"), "params: []\n").unwrap();

        // Act
        let diff = compute_diff(&old, &new).unwrap();

        // Assert — .git/HEAD must not appear as added
        assert!(diff.is_empty(), "unexpected diff: {diff:?}");
    }
}
