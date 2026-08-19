//! Read and decrypt LeetCode cookies straight from Chrome's cookie store.
//!
//! Replaces the archived `rookie` crate. leetctl needs a handful of cookies from one
//! browser, so this reuses the SQLite stack already pulled in by `diesel` to read the
//! cookie DB and implements just Chrome's value decryption — no second SQLite client and
//! no dbus linkage. The same Chrome install also names the `User-Agent` to present.
use crate::{Error, Result};
use cbc::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use diesel::{prelude::*, sql_query, sql_types, sqlite::SqliteConnection};
use std::{collections::HashMap, fmt::Display, path::PathBuf};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// Cookies worth forwarding to LeetCode. The first two are the identity; the rest are
/// Cloudflare's proof that this machine already passed a bot check. Dropping the Cloudflare
/// ones is what makes an *idle* session fail: `__cf_bm` lives about half an hour, and once it
/// lapses the next request is scored from scratch and gets a challenge page instead of a
/// judge result.
const FORWARDED_COOKIES: [&str; 5] = [
    "csrftoken",
    "LEETCODE_SESSION",
    "cf_clearance",
    "__cf_bm",
    "_cfuvid",
];

/// Chrome major version to claim when the local Chrome cannot be interrogated. Only a
/// fallback: a stale version is itself a bot signal, so detection is always tried first.
const FALLBACK_CHROME_MAJOR: u32 = 151;

/// Resolved LeetCode identity.
#[derive(Debug)]
pub struct Ident {
    pub csrf: String,
    session: String,
    /// Cloudflare clearance cookies as `(name, value)`, when Chrome had them. Empty is not
    /// an error — a machine that never saw a challenge has none.
    cloudflare: Vec<(String, String)>,
}

impl Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LEETCODE_SESSION={};csrftoken={};",
            self.session, self.csrf
        )?;
        for (name, value) in &self.cloudflare {
            write!(f, "{name}={value};")?;
        }
        Ok(())
    }
}

/// Get cookies, preferring the configured ones and falling back to Chrome.
pub fn cookies() -> Result<Ident> {
    let ccfg = crate::config::Config::locate()?.cookies;
    if !ccfg.csrf.is_empty() && !ccfg.session.is_empty() {
        return Ok(Ident {
            csrf: ccfg.csrf,
            session: ccfg.session,
            cloudflare: Vec::new(),
        });
    }

    let mut jar = chrome_cookies(&ccfg.site.to_string())?;
    let csrf = jar.remove("csrftoken").ok_or(Error::ChromeNotLogin)?;
    let session = jar
        .remove("LEETCODE_SESSION")
        .ok_or(Error::ChromeNotLogin)?;
    Ok(Ident {
        csrf,
        session,
        cloudflare: jar.into_iter().collect(),
    })
}

/// The `User-Agent` to present to LeetCode.
///
/// Cloudflare scores a request that sends no `User-Agent` at all as a bot and answers with a
/// challenge page, and it ties `cf_clearance` to the agent string that earned it — so this
/// has to name the same Chrome whose cookies [`cookies`] reads.
pub fn user_agent() -> String {
    let major = chrome_major_version().unwrap_or(FALLBACK_CHROME_MAJOR);
    format!(
        "Mozilla/5.0 ({CHROME_PLATFORM}) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{major}.0.0.0 Safari/537.36"
    )
}

/// Pull the major out of `151.0.7922.169` or `Google Chrome 151.0.7922.169`.
fn major_from_version(output: &str) -> Option<u32> {
    output
        .split_whitespace()
        .find_map(|word| word.split('.').next()?.parse().ok())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn chrome_major_version() -> Option<u32> {
    None
}

#[derive(QueryableByName)]
struct CookieRow {
    #[diesel(sql_type = sql_types::Text)]
    name: String,
    #[diesel(sql_type = sql_types::Binary)]
    encrypted_value: Vec<u8>,
}

/// Read Chrome's cookie DB for `domain` and decrypt the matching values.
fn chrome_cookies(domain: &str) -> Result<HashMap<String, String>> {
    let db = cookie_db_path().ok_or(Error::ChromeNotLogin)?;

    // Chrome keeps the DB locked while running; read from a snapshot copy.
    let tmp = std::env::temp_dir().join(format!("leetctl-{}.sqlite", std::process::id()));
    std::fs::copy(&db, &tmp)?;
    let mut conn = SqliteConnection::establish(&tmp.to_string_lossy())
        .map_err(|e| anyhow::anyhow!("open chrome cookies: {e}"))?;
    let rows: Vec<CookieRow> =
        sql_query("SELECT name, encrypted_value FROM cookies WHERE host_key LIKE ?")
            .bind::<sql_types::Text, _>(format!("%{domain}%"))
            .load(&mut conn)
            .map_err(|e| anyhow::anyhow!("read chrome cookies: {e}"))?;
    drop(conn);
    let _ = std::fs::remove_file(&tmp);

    let key = encryption_key()?;
    let mut jar = HashMap::new();
    for row in rows {
        if FORWARDED_COOKIES.contains(&row.name.as_str())
            && let Ok(value) = decrypt(&row.encrypted_value, &key)
        {
            jar.insert(row.name, value);
        }
    }
    Ok(jar)
}

/// Decrypt a Chrome `encrypted_value` (AES-128-CBC, `v10`/`v11`).
fn decrypt(blob: &[u8], key: &[u8]) -> Result<String> {
    if blob.len() <= 3 || (&blob[..3] != b"v10" && &blob[..3] != b"v11") {
        return Err(anyhow::anyhow!("unsupported cookie encryption").into());
    }
    let iv = [0x20u8; 16]; // Chrome uses 16 spaces as the IV.
    let plain = Aes128CbcDec::new_from_slices(key, &iv)
        .map_err(|e| anyhow::anyhow!("cipher init: {e}"))?
        .decrypt_padded_vec::<Pkcs7>(&blob[3..])
        .map_err(|e| anyhow::anyhow!("decrypt cookie: {e}"))?;
    Ok(finalize(plain))
}

/// Chrome (v104+) prepends a 32-byte SHA-256 of the domain to the plaintext;
/// strip it when the leading bytes aren't printable.
fn finalize(plain: Vec<u8>) -> String {
    if let Ok(s) = std::str::from_utf8(&plain)
        && !s.chars().any(|c| c.is_control())
    {
        return s.to_string();
    }
    if plain.len() > 32 {
        return String::from_utf8_lossy(&plain[32..]).into_owned();
    }
    String::from_utf8_lossy(&plain).into_owned()
}

/// PBKDF2-HMAC-SHA1 over the OS storage password (Chrome's KDF).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn derive(password: &[u8], iterations: u32) -> Vec<u8> {
    pbkdf2::pbkdf2_hmac_array::<sha1::Sha1, 16>(password, b"saltysalt", iterations).to_vec()
}

/// Platform token of the `User-Agent` string, matching what Chrome sends on this OS.
#[cfg(target_os = "macos")]
const CHROME_PLATFORM: &str = "Macintosh; Intel Mac OS X 10_15_7";

#[cfg(target_os = "macos")]
fn chrome_major_version() -> Option<u32> {
    let out = std::process::Command::new("defaults")
        .args([
            "read",
            "/Applications/Google Chrome.app/Contents/Info",
            "CFBundleShortVersionString",
        ])
        .output()
        .ok()?;
    major_from_version(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "macos")]
fn cookie_db_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join("Library/Application Support/Google/Chrome/Default/Cookies"))
}

#[cfg(target_os = "macos")]
fn encryption_key() -> Result<Vec<u8>> {
    let out = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-w",
            "-a",
            "Chrome",
            "-s",
            "Chrome Safe Storage",
        ])
        .output()?;
    let pw = String::from_utf8_lossy(&out.stdout);
    let pw = pw.trim();
    if pw.is_empty() {
        return Err(Error::ChromeNotLogin);
    }
    Ok(derive(pw.as_bytes(), 1003))
}

#[cfg(target_os = "linux")]
const CHROME_PLATFORM: &str = "X11; Linux x86_64";

#[cfg(target_os = "linux")]
fn chrome_major_version() -> Option<u32> {
    ["google-chrome", "chromium"].iter().find_map(|binary| {
        let out = std::process::Command::new(binary)
            .arg("--version")
            .output()
            .ok()?;
        major_from_version(&String::from_utf8_lossy(&out.stdout))
    })
}

#[cfg(target_os = "linux")]
fn cookie_db_path() -> Option<PathBuf> {
    let cfg = dirs::config_dir()?;
    ["google-chrome", "chromium"]
        .iter()
        .map(|b| cfg.join(b).join("Default/Cookies"))
        .find(|p| p.exists())
}

#[cfg(target_os = "linux")]
fn encryption_key() -> Result<Vec<u8>> {
    // Prefer the Secret Service password; fall back to Chrome's default.
    let pw = std::process::Command::new("secret-tool")
        .args(["lookup", "application", "chrome"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "peanuts".to_string());
    Ok(derive(pw.as_bytes(), 1))
}

#[cfg(target_os = "windows")]
const CHROME_PLATFORM: &str = "Windows NT 10.0; Win64; x64";

#[cfg(target_os = "windows")]
fn cookie_db_path() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("Google/Chrome/User Data/Default/Network/Cookies"))
}

#[cfg(target_os = "windows")]
fn encryption_key() -> Result<Vec<u8>> {
    // Windows uses DPAPI + AES-256-GCM (and app-bound encryption on Chrome
    // 127+), a separate code path that isn't implemented yet. Until then,
    // Windows users should set `csrf`/`session` in the config manually.
    Err(Error::ChromeNotLogin)
}

// for headless installations stub functions are defined
// can add to stubs with cfg(any(...))
#[cfg(target_os = "android")]
const CHROME_PLATFORM: &str = "Linux; Android 10; K";

#[cfg(target_os = "android")]
fn cookie_db_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "android")]
fn encryption_key() -> Result<Vec<u8>> {
    Err(Error::ChromeNotLogin)
}

#[cfg(test)]
mod tests {
    use super::major_from_version;

    #[test]
    fn major_from_version_reads_a_bare_version() {
        assert_eq!(major_from_version("151.0.7922.169\n"), Some(151));
    }

    #[test]
    fn major_from_version_skips_the_product_name() {
        assert_eq!(
            major_from_version("Google Chrome 151.0.7922.169\n"),
            Some(151)
        );
    }

    #[test]
    fn major_from_version_rejects_output_without_a_version() {
        assert_eq!(major_from_version("command not found"), None);
    }
}
