use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

const REPO: &str = "timutti/cwinner";

pub fn update(binary_path: &Path) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {current_version}");

    // Fetch latest release tag from GitHub
    let output = Command::new("curl")
        .args([
            "-s",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("failed to run curl — is it installed?")?;

    if !output.status.success() {
        bail!("failed to fetch latest release from GitHub");
    }

    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse GitHub API response")?;

    let tag = body["tag_name"]
        .as_str()
        .context("no tag_name in GitHub release")?;

    let latest_version = tag.strip_prefix('v').unwrap_or(tag);

    if latest_version == current_version {
        println!("Already up to date ({current_version}).");
        return Ok(());
    }

    println!("New version available: {latest_version}");

    // Detect OS
    let uname_s = cmd_stdout("uname", &["-s"])?;
    let os = match uname_s.trim() {
        "Linux" => "unknown-linux-gnu",
        "Darwin" => "apple-darwin",
        other => bail!("unsupported OS: {other}"),
    };

    // Detect architecture
    let uname_m = cmd_stdout("uname", &["-m"])?;
    let arch = match uname_m.trim() {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        other => bail!("unsupported architecture: {other}"),
    };

    let target = format!("{arch}-{os}");
    let url = format!("https://github.com/{REPO}/releases/download/{tag}/cwinner-{target}.tar.gz");

    // Download to unique temp dir (PID avoids collisions between concurrent runs)
    let tmp_dir = std::env::temp_dir().join(format!("cwinner-update-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir)?;

    let tarball = tmp_dir.join("cwinner.tar.gz");
    println!("Downloading {url} ...");
    let status = Command::new("curl")
        .args(["-fsSL", "-o", tarball.to_str().unwrap(), &url])
        .status()
        .context("failed to run curl")?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        bail!("download failed for {target}");
    }

    // Verify the download against the published SHA-256 before trusting it.
    let checksum_url = format!("{url}.sha256");
    let verify = || -> Result<()> {
        let body = Command::new("curl")
            .args(["-fsSL", &checksum_url])
            .output()
            .context("failed to run curl for checksum")?;
        if !body.status.success() {
            bail!("could not download checksum from {checksum_url}");
        }
        let expected = parse_sha256_digest(&String::from_utf8_lossy(&body.stdout))
            .context("malformed checksum file")?;
        let actual = sha256_of_file(&tarball)?;
        if actual != expected {
            bail!("checksum mismatch: expected {expected}, got {actual}");
        }
        Ok(())
    };
    if let Err(e) = verify() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e.context("refusing to install an unverified binary"));
    }
    println!("Checksum verified.");

    // Extract
    let status = Command::new("tar")
        .args([
            "xzf",
            tarball.to_str().unwrap(),
            "-C",
            tmp_dir.to_str().unwrap(),
        ])
        .status()
        .context("failed to run tar")?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        bail!("extraction failed");
    }

    // Replace current binary
    let new_binary = tmp_dir.join("cwinner");
    if !new_binary.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        bail!("extracted archive does not contain 'cwinner' binary");
    }

    // Stop the daemon before replacing the binary so in-flight state is flushed to disk
    stop_daemon();

    let target_path = std::env::current_exe().unwrap_or_else(|_| binary_path.to_path_buf());

    // Remove the old binary before copying so the OS allocates a fresh inode.
    // On macOS, overwriting in-place (same inode) causes the kernel VFS cache
    // to serve stale executable metadata, which makes the new binary hang.
    // On Linux this is harmless but equally correct.
    let _ = std::fs::remove_file(&target_path);
    std::fs::copy(&new_binary, &target_path)
        .with_context(|| format!("failed to replace binary at {}", target_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Re-run install to update hooks, daemon, sounds
    println!("Running install to update hooks and daemon...");
    let status = Command::new(target_path.as_os_str())
        .arg("install")
        .status()
        .context("failed to run cwinner install")?;
    if !status.success() {
        bail!("cwinner install failed after update");
    }

    println!("\nUpdated cwinner to {latest_version}!");
    Ok(())
}

fn stop_daemon() {
    // Kill the daemon process (auto-starts from hooks on next event)
    let _ = Command::new("pkill")
        .args(["-f", "cwinner daemon"])
        .status();

    // macOS: also unload launchd agent if present
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("launchctl")
            .args([
                "unload",
                &format!(
                    "{}/Library/LaunchAgents/com.cwinner.daemon.plist",
                    std::env::var("HOME").unwrap_or_default()
                ),
            ])
            .status();
    }
}

fn cmd_stdout(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse the hex digest from a `sha256sum` / `shasum -a 256` line, which looks
/// like "<64-hex-chars>  <filename>". Returns the lowercased digest, or None if
/// the first token isn't a valid SHA-256 hex string.
fn parse_sha256_digest(checksum_file: &str) -> Option<String> {
    let token = checksum_file.split_whitespace().next()?;
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_ascii_lowercase())
    } else {
        None
    }
}

/// Compute the SHA-256 of a file using the system tool (`sha256sum` on Linux,
/// `shasum -a 256` on macOS) and return the lowercase hex digest.
fn sha256_of_file(path: &Path) -> Result<String> {
    let path_str = path.to_str().context("non-UTF-8 path")?;
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("shasum", vec!["-a", "256", path_str])
    } else {
        ("sha256sum", vec![path_str])
    };
    let output = Command::new(cmd)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run {cmd}"))?;
    if !output.status.success() {
        bail!("{cmd} failed to hash {path_str}");
    }
    parse_sha256_digest(&String::from_utf8_lossy(&output.stdout))
        .context("could not parse checksum tool output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sha256_digest_valid() {
        let line = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  cwinner-x86_64-apple-darwin.tar.gz";
        assert_eq!(
            parse_sha256_digest(line).as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }

    #[test]
    fn test_parse_sha256_digest_normalizes_case() {
        let line = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789  f";
        assert_eq!(
            parse_sha256_digest(line).as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
    }

    #[test]
    fn test_parse_sha256_digest_rejects_garbage() {
        assert_eq!(parse_sha256_digest(""), None);
        assert_eq!(parse_sha256_digest("not-a-hash file"), None);
        assert_eq!(parse_sha256_digest("deadbeef short"), None);
    }

    #[test]
    fn test_sha256_of_file_matches_known_vector() {
        // SHA-256 of the bytes "hello" (no trailing newline) is well-known.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("data");
        std::fs::write(&f, b"hello").unwrap();
        assert_eq!(
            sha256_of_file(&f).unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
