use anyhow::{Context, Result};
use std::path::Path;

pub fn install(binary_path: &Path) -> Result<()> {
    let binary_str = binary_path.to_str().unwrap_or("cwinner");

    // 1. Claude Code settings
    let claude_settings = dirs::home_dir()
        .context("no home dir")?
        .join(".claude")
        .join("settings.json");
    if claude_settings.exists() {
        add_claude_hooks(&claude_settings, binary_str)?;
        println!("✓ Claude Code hooks přidány do {}", claude_settings.display());
    } else {
        println!("⚠ ~/.claude/settings.json nenalezen — přidej hooks ručně");
    }

    // 2. Git global hooks
    let git_hooks_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("git")
        .join("hooks");
    std::fs::create_dir_all(&git_hooks_dir)?;
    install_git_hook(
        &git_hooks_dir.join("post-commit"),
        include_str!("hooks/templates/git_post_commit.sh"),
    )?;
    install_git_hook(
        &git_hooks_dir.join("pre-push"),
        include_str!("hooks/templates/git_post_push.sh"),
    )?;
    println!("✓ Git hooks nainstalovány do {}", git_hooks_dir.display());

    // 3. Default config
    let config_dir = dirs::config_dir()
        .context("no config dir")?
        .join("cwinner");
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("✓ Konfigurace vytvořena v {}", config_path.display());
    }

    // 4. Extract bundled WAV sounds
    let sounds_dir = config_dir.join("sounds").join("default");
    crate::sounds::extract_all_sounds(&sounds_dir)
        .context("Failed to extract default sound pack")?;
    println!("  Sound pack extracted to {}", sounds_dir.display());

    // 5. State dir
    let state_dir = dirs::data_local_dir()
        .context("no data dir")?
        .join("cwinner");
    std::fs::create_dir_all(&state_dir)?;

    // 6. Systemd / launchd
    register_service(binary_str)?;

    println!("\n🎉 cwinner nainstalován! Spusť: cwinner status");
    Ok(())
}

pub fn add_claude_hooks(settings_path: &Path, binary: &str) -> Result<()> {
    let content = std::fs::read_to_string(settings_path).unwrap_or_else(|_| "{}".into());
    let mut v: serde_json::Value =
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

    // Zajisti že hooks objekt existuje
    if !v["hooks"].is_object() {
        v["hooks"] = serde_json::json!({});
    }

    let hooks_to_add = [
        ("PostToolUse", format!("{} hook post-tool-use", binary)),
        ("TaskCompleted", format!("{} hook task-completed", binary)),
        ("Stop", format!("{} hook session-end", binary)),
    ];

    for (hook_name, cmd) in &hooks_to_add {
        // Zajisti že pole existuje
        if !v["hooks"][hook_name].is_array() {
            v["hooks"][hook_name] = serde_json::json!([]);
        }

        // Odstraň staré záznamy formátu {"cmd": "...cwinner..."} (migrační čištění)
        if let Some(arr) = v["hooks"][hook_name].as_array_mut() {
            arr.retain(|h| {
                !h["cmd"]
                    .as_str()
                    .map(|s| s.contains("cwinner"))
                    .unwrap_or(false)
            });
        }

        // Přidej pouze pokud cwinner hook ještě není
        let already_present = v["hooks"][hook_name]
            .as_array()
            .map(|arr| {
                arr.iter().any(|h| {
                    h["hooks"]
                        .as_array()
                        .map(|inner| {
                            inner.iter().any(|e| {
                                e["command"]
                                    .as_str()
                                    .map(|s| s.contains("cwinner"))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if !already_present {
            v["hooks"][hook_name]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "hooks": [{"type": "command", "command": cmd}]
                }));
        }
    }

    std::fs::write(settings_path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

fn install_git_hook(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn register_service(binary: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    register_launchd(binary)?;
    #[cfg(target_os = "linux")]
    register_systemd(binary)?;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = binary;
        println!("⚠ Automatická registrace service není podporována na této platformě");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn register_systemd(binary: &str) -> Result<()> {
    let service_dir = dirs::home_dir()
        .context("no home")?
        .join(".config/systemd/user");
    std::fs::create_dir_all(&service_dir)?;
    let unit = format!(
        "[Unit]\nDescription=cwinner celebration daemon\nAfter=default.target\n\n[Service]\nExecStart={binary} daemon\nRestart=on-failure\nRestartSec=3\n\n[Install]\nWantedBy=default.target\n"
    );
    std::fs::write(service_dir.join("cwinner.service"), unit)?;
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "cwinner"])
        .status();
    println!("✓ systemd user service registrován");
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_launchd(binary: &str) -> Result<()> {
    let plist_dir = dirs::home_dir()
        .context("no home")?
        .join("Library/LaunchAgents");
    std::fs::create_dir_all(&plist_dir)?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.cwinner.daemon</string>
  <key>ProgramArguments</key>
  <array><string>{binary}</string><string>daemon</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict>
</plist>"#
    );
    let plist_path = plist_dir.join("com.cwinner.daemon.plist");
    std::fs::write(&plist_path, plist)?;
    let _ = std::process::Command::new("launchctl")
        .args(["load", plist_path.to_str().unwrap_or("")])
        .status();
    println!("✓ launchd agent registrován");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "cwinner"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "cwinner"])
            .status();
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(plist) = dirs::home_dir()
            .map(|h| h.join("Library/LaunchAgents/com.cwinner.daemon.plist"))
        {
            if plist.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", plist.to_str().unwrap_or("")])
                    .status();
            }
        }
    }
    println!("✓ cwinner odinstalován");
    Ok(())
}

const DEFAULT_CONFIG: &str = r#"[intensity]
routine = "off"
milestone = "medium"
breakthrough = "epic"

[audio]
enabled = true
sound_pack = "default"
volume = 0.8

[visual]
confetti = true
splash_screen = true
progress_bar = true
confetti_duration_ms = 1500
splash_duration_ms = 2000
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_merge_claude_settings_empty() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, "{}").unwrap();

        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v["hooks"].is_object());
        assert!(v["hooks"]["PostToolUse"].is_array());
    }

    #[test]
    fn test_merge_claude_settings_existing_hooks() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(
            &settings_path,
            r#"{"hooks":{"PostToolUse":[{"cmd":"existing"}]}}"#,
        )
        .unwrap();

        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v["hooks"]["PostToolUse"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn test_no_duplicate_hooks() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, "{}").unwrap();

        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();
        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let hooks = v["hooks"]["PostToolUse"].as_array().unwrap();
        let cwinner_count = hooks
            .iter()
            .filter(|h| {
                h["hooks"].as_array().map(|inner| {
                    inner.iter().any(|e| {
                        e["command"].as_str().map(|s| s.contains("cwinner")).unwrap_or(false)
                    })
                }).unwrap_or(false)
            })
            .count();
        assert_eq!(cwinner_count, 1);
    }

    #[test]
    fn test_install_creates_wav_sounds() {
        let tmp = tempdir().unwrap();
        let sounds_dir = tmp.path().join("sounds/default");
        crate::sounds::extract_all_sounds(&sounds_dir).unwrap();
        assert!(sounds_dir.join("mini.wav").exists());
        assert!(sounds_dir.join("epic.wav").exists());
    }
}
