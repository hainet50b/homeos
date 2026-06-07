use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CACHE_FILENAME: &str = ".last-update-check";
const CACHE_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;
const FETCH_TIMEOUT_MS: u64 = 1500;
const RELEASES_URL: &str = "https://api.github.com/repos/hainet50b/homeos/releases/latest";
const UPDATE_URL: &str = "https://github.com/hainet50b/homeos";
const SKIP_ENV_VAR: &str = "HOMEOS_SKIP_UPDATE_CHECK";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct UpdateCheckCache {
    pub last_checked_at: u64,
    pub latest_tag: String,
}

pub fn cache_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CACHE_FILENAME)
}

pub fn current_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Seed the cache with the current binary's tag and a now timestamp without
/// making any network call. Called by `homeos init` so a freshly initialized
/// data directory has a primed cache and the user is not pinged within their
/// first 7-day window.
pub fn seed_cache(data_dir: &Path) -> std::io::Result<()> {
    let cache = UpdateCheckCache {
        last_checked_at: now_seconds(),
        latest_tag: current_tag(),
    };
    write_cache(&cache_path(data_dir), &cache)
}

/// Best-effort update check. Reads the cache, fetches when stale or missing,
/// writes back the result, and emits one stderr line when a newer release is
/// available. Honors `HOMEOS_SKIP_UPDATE_CHECK` (any non-empty value disables
/// both the cache read and the network call, with no file write either).
pub fn check_and_notify(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if skip_check() {
        return Ok(());
    }
    check_and_notify_to(
        data_dir,
        &mut std::io::stderr(),
        default_fetch,
        now_seconds(),
    )
}

fn skip_check() -> bool {
    std::env::var_os(SKIP_ENV_VAR).is_some_and(|v| !v.is_empty())
}

fn default_fetch() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(FETCH_TIMEOUT_MS)))
        .build()
        .into();
    let response: serde_json::Value = agent
        .get(RELEASES_URL)
        .header("User-Agent", "homeos")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    response.get("tag_name")?.as_str().map(|s| s.to_string())
}

fn read_cache(path: &Path) -> Option<UpdateCheckCache> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(path: &Path, cache: &UpdateCheckCache) -> std::io::Result<()> {
    let json = serde_json::to_string(cache).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

fn check_and_notify_to<W, F>(
    data_dir: &Path,
    writer: &mut W,
    fetch: F,
    now: u64,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: Write,
    F: FnOnce() -> Option<String>,
{
    let path = cache_path(data_dir);
    let previous = read_cache(&path);

    let latest_tag = match previous {
        Some(c) if now.saturating_sub(c.last_checked_at) < CACHE_TTL_SECONDS => {
            // Fresh — reuse without a network call or file write.
            c.latest_tag
        }
        Some(c) => {
            // Stale — fetch and rewrite. Preserve previous tag on fetch failure.
            let fetched = fetch().unwrap_or(c.latest_tag);
            let _ = write_cache(
                &path,
                &UpdateCheckCache {
                    last_checked_at: now,
                    latest_tag: fetched.clone(),
                },
            );
            fetched
        }
        None => {
            // Missing or unparseable — fetch and write. Seed with current tag on failure.
            let fetched = fetch().unwrap_or_else(current_tag);
            let _ = write_cache(
                &path,
                &UpdateCheckCache {
                    last_checked_at: now,
                    latest_tag: fetched.clone(),
                },
            );
            fetched
        }
    };

    if is_update_available(&latest_tag, &current_tag()) {
        writeln!(
            writer,
            "homeos: {latest_tag} available — update at {UPDATE_URL}"
        )?;
    }
    Ok(())
}

/// Parse a `vX.Y.Z` release tag into a numeric `(major, minor, patch)` triple.
/// Returns `None` for anything that does not match the exact form (missing `v`
/// prefix, wrong component count, or non-numeric components).
fn parse_tag(tag: &str) -> Option<(u64, u64, u64)> {
    let rest = tag.strip_prefix('v')?;
    let mut parts = rest.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True when `latest` is a strictly newer release than `current`, comparing
/// `vX.Y.Z` numerically. Any unparseable tag yields `false` — the check is
/// best-effort and silence beats a false alarm.
fn is_update_available(latest: &str, current: &str) -> bool {
    match (parse_tag(latest), parse_tag(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_test::EnvVarGuard;
    use tempfile::TempDir;

    fn fresh_timestamp(now: u64) -> u64 {
        now - 60
    }

    fn stale_timestamp(now: u64) -> u64 {
        now - CACHE_TTL_SECONDS - 1
    }

    #[test]
    fn test_seed_cache_writes_current_tag() {
        // Arrange
        let tmp = TempDir::new().unwrap();

        // Act
        seed_cache(tmp.path()).unwrap();

        // Assert
        let cache = read_cache(&cache_path(tmp.path())).expect("cache should be readable");
        assert_eq!(cache.latest_tag, current_tag());
        assert!(cache.last_checked_at > 0);
    }

    #[test]
    fn test_cache_hit_still_fresh_makes_no_network_call() {
        // Arrange — write a fresh cache and assert the fetch closure is never invoked.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: fresh_timestamp(now),
                latest_tag: current_tag(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || -> Option<String> {
            panic!("fetch must not be called on a fresh cache hit");
        };

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert — no stderr notice, and the cache timestamp is unchanged (no rewrite).
        assert!(writer.is_empty(), "stderr should be silent on cache hit");
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.last_checked_at, fresh_timestamp(now));
    }

    #[test]
    fn test_cache_miss_fetch_success_writes_fetched_tag() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v99.0.0".to_string());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, "v99.0.0");
        assert_eq!(cache.last_checked_at, now);
        let notice = String::from_utf8(writer).unwrap();
        assert!(
            notice.contains("v99.0.0 available"),
            "notice should mention the newer tag, got: {notice:?}"
        );
    }

    #[test]
    fn test_cache_stale_fetch_success_rewrites_with_fetched_tag() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: stale_timestamp(now),
                latest_tag: current_tag(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v99.0.0".to_string());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, "v99.0.0");
        assert_eq!(cache.last_checked_at, now);
    }

    #[test]
    fn test_cache_miss_fetch_timeout_writes_current_tag_with_now_timestamp() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || None; // Simulates timeout / parse / DNS failure.

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, current_tag());
        assert_eq!(cache.last_checked_at, now);
        assert!(
            writer.is_empty(),
            "no notice when fallback equals current tag"
        );
    }

    #[test]
    fn test_cache_stale_fetch_timeout_preserves_previous_tag_with_now_timestamp() {
        // Arrange — stale cache holds a known newer tag; fetch fails.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: stale_timestamp(now),
                latest_tag: "v99.0.0".to_string(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || None;

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, "v99.0.0");
        assert_eq!(cache.last_checked_at, now);
        let notice = String::from_utf8(writer).unwrap();
        assert!(notice.contains("v99.0.0"));
    }

    #[test]
    fn test_corrupt_json_is_treated_as_cache_miss() {
        // Arrange — pre-existing file with garbage content.
        let tmp = TempDir::new().unwrap();
        fs::write(cache_path(tmp.path()), "not json at all").unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v99.0.0".to_string());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert — the corrupt file is overwritten with a valid cache.
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, "v99.0.0");
        assert_eq!(cache.last_checked_at, now);
    }

    #[test]
    fn test_skip_env_var_disables_both_cache_read_and_file_write() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let guard = EnvVarGuard::capture(SKIP_ENV_VAR);
        guard.set("1");

        // Act
        check_and_notify(tmp.path()).unwrap();

        // Assert — no cache file was created and no notice was emitted.
        assert!(!cache_path(tmp.path()).exists());
    }

    #[test]
    fn test_current_tag_no_warning_emitted() {
        // Arrange — cached tag equals current binary's tag.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: fresh_timestamp(now),
                latest_tag: current_tag(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || -> Option<String> {
            panic!("fetch must not be called on a fresh cache hit");
        };

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        assert!(
            writer.is_empty(),
            "no stderr notice when current tag matches latest"
        );
    }

    #[test]
    fn test_notice_format_matches_spec() {
        // Arrange
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v99.0.0".to_string());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert — single line `homeos: v<latest> available — update at <url>`.
        let notice = String::from_utf8(writer).unwrap();
        assert_eq!(
            notice,
            format!("homeos: v99.0.0 available — update at {UPDATE_URL}\n")
        );
    }

    #[test]
    fn test_latest_newer_notifies() {
        // Arrange — fetched tag is one patch ahead of the current binary.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let current = current_tag();
        let (major, minor, patch) = parse_tag(&current).unwrap();
        let newer = format!("v{major}.{minor}.{}", patch + 1);
        let newer_for_fetch = newer.clone();
        let fetch = move || Some(newer_for_fetch.clone());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        let notice = String::from_utf8(writer).unwrap();
        assert!(
            notice.contains(&format!("{newer} available")),
            "a strictly newer tag must notify, got: {notice:?}"
        );
    }

    #[test]
    fn test_equal_tag_is_silent() {
        // Arrange — fresh cache holds exactly the current binary's tag.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: fresh_timestamp(now),
                latest_tag: current_tag(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || -> Option<String> {
            panic!("fetch must not be called on a fresh cache hit");
        };

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        assert!(writer.is_empty(), "equal tag must not notify");
    }

    #[test]
    fn test_latest_older_than_current_is_silent() {
        // Arrange — fresh cache holds a tag OLDER than the current binary, the
        // post-upgrade stale-cache case. No fetch happens on a fresh cache.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        write_cache(
            &cache_path(tmp.path()),
            &UpdateCheckCache {
                last_checked_at: fresh_timestamp(now),
                latest_tag: "v0.0.1".to_string(),
            },
        )
        .unwrap();
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || -> Option<String> {
            panic!("fetch must not be called on a fresh cache hit");
        };

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert
        assert!(writer.is_empty(), "an older cached tag must not notify");
    }

    #[test]
    fn test_unparseable_fetched_tag_is_silent() {
        // Arrange — fetch returns a tag that is not in vX.Y.Z form.
        let tmp = TempDir::new().unwrap();
        let now = 1_700_000_000u64;
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("nightly".to_string());

        // Act
        check_and_notify_to(tmp.path(), &mut writer, fetch, now).unwrap();

        // Assert — cache is still written, but no notice is emitted.
        let cache = read_cache(&cache_path(tmp.path())).unwrap();
        assert_eq!(cache.latest_tag, "nightly");
        assert!(writer.is_empty(), "an unparseable tag must not notify");
    }

    #[test]
    fn test_parse_tag_accepts_well_formed() {
        // Arrange / Act / Assert
        assert_eq!(parse_tag("v0.3.12"), Some((0, 3, 12)));
        assert_eq!(parse_tag("v10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn test_parse_tag_rejects_malformed() {
        // Arrange / Act / Assert
        assert_eq!(parse_tag("0.3.12"), None); // missing v prefix
        assert_eq!(parse_tag("v0.3"), None); // too few components
        assert_eq!(parse_tag("v0.3.12.1"), None); // too many components
        assert_eq!(parse_tag("vx.y.z"), None); // non-numeric
        assert_eq!(parse_tag("nightly"), None); // not a version at all
    }

    #[test]
    fn test_is_update_available_compares_strictly_newer() {
        // Arrange / Act / Assert
        assert!(is_update_available("v0.3.13", "v0.3.12"));
        assert!(is_update_available("v0.4.0", "v0.3.12"));
        assert!(is_update_available("v1.0.0", "v0.3.12"));
        assert!(!is_update_available("v0.3.12", "v0.3.12")); // equal
        assert!(!is_update_available("v0.3.11", "v0.3.12")); // older
        assert!(!is_update_available("garbage", "v0.3.12")); // unparseable latest
        assert!(!is_update_available("v0.3.13", "garbage")); // unparseable current
    }
}
