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
    std::fs::copy(&new_binary, &target_path)
        .with_context(|| format!("failed to replace binary at {}", target_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // macOS: clear quarantine xattr and ad-hoc codesign so Gatekeeper
    // doesn't block/hang the binary on first launch
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("xattr")
            .args(["-cr", target_path.to_str().unwrap_or("")])
            .status();
        let _ = Command::new("codesign")
            .args(["-s", "-", target_path.to_str().unwrap_or("")])
            .status();
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
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "cwinner"])
            .status();
    }
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
