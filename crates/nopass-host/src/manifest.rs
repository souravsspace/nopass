//! Native messaging manifests.
//!
//! A browser will only launch this host if a manifest naming it sits in a
//! browser-specific directory, and it will only hand it to the extension IDs
//! that manifest lists. That list is the access control for the whole bridge,
//! so it is generated from the ID the caller supplies rather than hand-edited
//! (ADR-0001).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

/// The name the extension passes to `connectNative`.
pub const HOST_NAME: &str = "com.nopass.host";

const DESCRIPTION: &str = "Reads the local nopass store for the nopass browser extension";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Chrome,
    Chromium,
    Brave,
    Edge,
    Firefox,
}

impl Browser {
    pub const ALL: &'static [Self] =
        &[Self::Chrome, Self::Chromium, Self::Brave, Self::Edge, Self::Firefox];

    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "chrome" => Some(Self::Chrome),
            "chromium" => Some(Self::Chromium),
            "brave" => Some(Self::Brave),
            "edge" => Some(Self::Edge),
            "firefox" => Some(Self::Firefox),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Brave => "brave",
            Self::Edge => "edge",
            Self::Firefox => "firefox",
        }
    }

    /// Firefox names extensions by ID; the Chromium family names them by
    /// origin. The two keys are not interchangeable.
    fn is_gecko(self) -> bool {
        self == Self::Firefox
    }

    /// Where this browser looks for per-user manifests. `None` on a platform
    /// nopass does not support.
    pub fn manifest_dir(self, home: &Path) -> Option<PathBuf> {
        let relative = if cfg!(target_os = "macos") {
            match self {
                Self::Chrome => "Library/Application Support/Google/Chrome/NativeMessagingHosts",
                Self::Chromium => "Library/Application Support/Chromium/NativeMessagingHosts",
                Self::Brave => {
                    "Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"
                }
                Self::Edge => "Library/Application Support/Microsoft Edge/NativeMessagingHosts",
                Self::Firefox => "Library/Application Support/Mozilla/NativeMessagingHosts",
            }
        } else if cfg!(target_os = "linux") {
            match self {
                Self::Chrome => ".config/google-chrome/NativeMessagingHosts",
                Self::Chromium => ".config/chromium/NativeMessagingHosts",
                Self::Brave => ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                Self::Edge => ".config/microsoft-edge/NativeMessagingHosts",
                Self::Firefox => ".mozilla/native-messaging-hosts",
            }
        } else {
            return None;
        };
        Some(home.join(relative))
    }
}

/// The manifest body. `extension` is a Chromium extension ID or a Gecko
/// add-on ID depending on the browser.
pub fn body(browser: Browser, host_binary: &Path, extension: &str) -> Value {
    let mut manifest = json!({
        "name": HOST_NAME,
        "description": DESCRIPTION,
        "path": host_binary.display().to_string(),
        "type": "stdio",
    });

    let key = if browser.is_gecko() { "allowed_extensions" } else { "allowed_origins" };
    let value = if browser.is_gecko() {
        json!([extension])
    } else {
        json!([format!("chrome-extension://{extension}/")])
    };
    manifest[key] = value;
    manifest
}

/// Write the manifest for `browser`, returning where it went.
pub fn install(
    browser: Browser,
    home: &Path,
    host_binary: &Path,
    extension: &str,
) -> Result<PathBuf> {
    let dir = browser
        .manifest_dir(home)
        .with_context(|| format!("{} is not supported on this platform", browser.name()))?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("could not create {}", dir.display()))?;

    let file = dir.join(format!("{HOST_NAME}.json"));
    let text = serde_json::to_string_pretty(&body(browser, host_binary, extension))?;
    std::fs::write(&file, format!("{text}\n"))
        .with_context(|| format!("could not write {}", file.display()))?;
    Ok(file)
}

/// Remove the manifest for `browser`. `Ok(None)` if there was nothing there.
pub fn uninstall(browser: Browser, home: &Path) -> Result<Option<PathBuf>> {
    let Some(dir) = browser.manifest_dir(home) else {
        return Ok(None);
    };
    let file = dir.join(format!("{HOST_NAME}.json"));
    if !file.exists() {
        return Ok(None);
    }
    std::fs::remove_file(&file)?;
    Ok(Some(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_chromium_manifest_pins_an_extension_origin() {
        let manifest = body(Browser::Chrome, Path::new("/usr/bin/nopass-host"), "abcdef");

        assert_eq!(manifest["name"], HOST_NAME);
        assert_eq!(manifest["type"], "stdio");
        assert_eq!(manifest["path"], "/usr/bin/nopass-host");
        assert_eq!(manifest["allowed_origins"], json!(["chrome-extension://abcdef/"]));
        assert!(manifest.get("allowed_extensions").is_none());
    }

    #[test]
    fn a_firefox_manifest_pins_an_add_on_id() {
        let manifest =
            body(Browser::Firefox, Path::new("/usr/bin/nopass-host"), "nopass@example.org");

        assert_eq!(manifest["allowed_extensions"], json!(["nopass@example.org"]));
        assert!(manifest.get("allowed_origins").is_none());
    }

    #[test]
    fn every_browser_has_its_own_directory() {
        let home = Path::new("/home/sana");
        let dirs: Vec<_> =
            Browser::ALL.iter().filter_map(|b| b.manifest_dir(home)).collect();

        assert_eq!(dirs.len(), Browser::ALL.len(), "this platform is supported");
        let unique: std::collections::HashSet<_> = dirs.iter().collect();
        assert_eq!(unique.len(), dirs.len(), "two browsers share a directory: {dirs:?}");
        assert!(dirs.iter().all(|d| d.starts_with(home)));
    }

    #[test]
    fn names_round_trip() {
        for browser in Browser::ALL {
            assert_eq!(Browser::parse(browser.name()), Some(*browser));
        }
        assert_eq!(Browser::parse("netscape"), None);
    }

    #[test]
    fn installing_then_uninstalling_leaves_nothing_behind() {
        let home = TempDir::new().expect("a temporary home");
        let binary = Path::new("/usr/local/bin/nopass-host");

        let written = install(Browser::Chrome, home.path(), binary, "abcdef").expect("it installs");
        assert!(written.exists());

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
        assert_eq!(parsed["allowed_origins"], json!(["chrome-extension://abcdef/"]));

        assert_eq!(uninstall(Browser::Chrome, home.path()).unwrap(), Some(written.clone()));
        assert!(!written.exists());
        assert_eq!(uninstall(Browser::Chrome, home.path()).unwrap(), None);
    }

    #[test]
    fn installing_replaces_a_stale_manifest_rather_than_appending() {
        let home = TempDir::new().expect("a temporary home");
        let binary = Path::new("/usr/local/bin/nopass-host");

        install(Browser::Firefox, home.path(), binary, "old@example.org").unwrap();
        let written = install(Browser::Firefox, home.path(), binary, "new@example.org").unwrap();

        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
        assert_eq!(parsed["allowed_extensions"], json!(["new@example.org"]));
    }
}
