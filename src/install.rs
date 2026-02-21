use anyhow::{Context, Result};
use std::path::Path;

const HOOK_MARKER_START: &str = "# --- cwinner hook start ---";
const HOOK_MARKER_END: &str = "# --- cwinner hook end ---";

fn has_command(name: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {name}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn entry_has_cwinner(entry: &serde_json::Value) -> bool {
    entry["hooks"].as_array().is_some_and(|inner| {
        inner
            .iter()
            .any(|e| e["command"].as_str().is_some_and(|s| s.contains("cwinner")))
    })
}

fn entry_has_cwinner_legacy(entry: &serde_json::Value) -> bool {
    entry["cmd"].as_str().is_some_and(|s| s.contains("cwinner"))
}

pub fn install(binary_path: &Path) -> Result<()> {
    let binary_str = binary_path.to_str().unwrap_or("cwinner");

    // 1. Claude Code settings
    let claude_settings = dirs::home_dir()
        .context("no home dir")?
        .join(".claude")
        .join("settings.json");
    if claude_settings.exists() {
        add_claude_hooks(&claude_settings, binary_str)?;
        println!("✓ Claude Code hooks added to {}", claude_settings.display());
    } else {
        println!("⚠ ~/.claude/settings.json not found — add hooks manually");
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
        include_str!("hooks/templates/git_pre_push.sh"),
    )?;
    println!("✓ Git hooks installed to {}", git_hooks_dir.display());
    check_socket_tools();

    // 3. Default config
    let config_dir = dirs::config_dir().context("no config dir")?.join("cwinner");
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("✓ Config created at {}", config_path.display());
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

    println!("\n🎉 cwinner installed! Run: cwinner status");
    Ok(())
}

pub fn add_claude_hooks(settings_path: &Path, binary: &str) -> Result<()> {
    let content = std::fs::read_to_string(settings_path).unwrap_or_else(|_| "{}".into());
    let mut v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            // Back up malformed file instead of silently overwriting
            let backup = settings_path.with_extension("json.bak");
            std::fs::copy(settings_path, &backup)?;
            eprintln!(
                "warning: {} is malformed ({e}), backed up to {}",
                settings_path.display(),
                backup.display()
            );
            serde_json::json!({})
        }
    };

    // Ensure hooks object exists
    if !v["hooks"].is_object() {
        v["hooks"] = serde_json::json!({});
    }

    let hooks_to_add = [
        ("PostToolUse", format!("{} hook post-tool-use", binary)),
        ("TaskCompleted", format!("{} hook task-completed", binary)),
        ("Stop", format!("{} hook session-end", binary)),
    ];

    for (hook_name, cmd) in &hooks_to_add {
        // Ensure array exists
        if !v["hooks"][hook_name].is_array() {
            v["hooks"][hook_name] = serde_json::json!([]);
        }

        // Remove old entries with {"cmd": "...cwinner..."} format (migration cleanup)
        if let Some(arr) = v["hooks"][hook_name].as_array_mut() {
            arr.retain(|h| !entry_has_cwinner_legacy(h));
        }

        // Add only if cwinner hook is not already present
        let already_present = v["hooks"][hook_name]
            .as_array()
            .is_some_and(|arr| arr.iter().any(entry_has_cwinner));

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

/// Strip the shebang line from template content (the outer file manages it).
fn strip_shebang(content: &str) -> &str {
    if content.starts_with("#!") {
        content.find('\n').map(|i| &content[i + 1..]).unwrap_or("")
    } else {
        content
    }
}

fn install_git_hook(path: &Path, template: &str) -> Result<()> {
    let section = format!(
        "{}\n{}{}\n",
        HOOK_MARKER_START,
        strip_shebang(template),
        HOOK_MARKER_END
    );

    let new_content = if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if let (Some(start), Some(end)) = (
            existing.find(HOOK_MARKER_START),
            existing.find(HOOK_MARKER_END),
        ) {
            // Replace existing marked section
            let end_of_marker = end + HOOK_MARKER_END.len();
            let after = existing[end_of_marker..]
                .strip_prefix('\n')
                .unwrap_or(&existing[end_of_marker..]);
            format!("{}{}{}", &existing[..start], section, after)
        } else if existing.contains("cwinner") {
            // Legacy cwinner hook without markers — overwrite entirely
            format!("#!/usr/bin/env bash\n{}", section)
        } else {
            // Existing non-cwinner hook — append our section
            let mut base = existing.clone();
            if !base.ends_with('\n') {
                base.push('\n');
            }
            base.push('\n');
            base.push_str(&section);
            base
        }
    } else {
        format!("#!/usr/bin/env bash\n{}", section)
    };

    std::fs::write(path, new_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn check_socket_tools() {
    if !has_command("socat") && !has_command("nc") {
        println!("⚠ Neither socat nor nc found — git hooks won't be able to send events");
        if cfg!(target_os = "macos") {
            println!("  Install with: brew install socat");
        } else {
            println!("  Install with: sudo apt install socat  (or: sudo dnf install socat)");
        }
    }
}

fn register_service(binary: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    register_launchd(binary)?;
    #[cfg(target_os = "linux")]
    register_systemd(binary)?;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = binary;
        println!("⚠ Automatic service registration is not supported on this platform");
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
    let active = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "cwinner"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if active {
        println!("✓ systemd user service registered and running");
    } else {
        println!("✓ systemd user service registered");
        println!(
            "⚠ Service does not appear to be running — check: systemctl --user status cwinner"
        );
    }
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
    let active = std::process::Command::new("launchctl")
        .args(["list", "com.cwinner.daemon"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if active {
        println!("✓ launchd agent registered and running");
    } else {
        println!("✓ launchd agent registered");
        println!(
            "⚠ Agent does not appear to be running — check: launchctl list com.cwinner.daemon"
        );
    }
    Ok(())
}

pub fn uninstall() -> Result<()> {
    // 1. Stop + disable service
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "cwinner"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "cwinner"])
            .status();
        if let Some(unit) = dirs::home_dir().map(|h| h.join(".config/systemd/user/cwinner.service"))
        {
            if unit.exists() {
                std::fs::remove_file(&unit)?;
                println!("✓ Removed {}", unit.display());
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .status();
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(plist) =
            dirs::home_dir().map(|h| h.join("Library/LaunchAgents/com.cwinner.daemon.plist"))
        {
            if plist.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", plist.to_str().unwrap_or("")])
                    .status();
                std::fs::remove_file(&plist)?;
                println!("✓ Removed {}", plist.display());
            }
        }
    }

    // 2. Remove cwinner hooks from Claude Code settings
    let claude_settings = dirs::home_dir().map(|h| h.join(".claude").join("settings.json"));
    if let Some(ref path) = claude_settings {
        if path.exists() {
            remove_claude_hooks(path)?;
            println!("✓ Removed cwinner hooks from {}", path.display());
        }
    }

    // 3. Remove cwinner sections from git hooks
    let git_hooks_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("git")
        .join("hooks");
    for hook_name in &["post-commit", "pre-push"] {
        let hook_path = git_hooks_dir.join(hook_name);
        if hook_path.exists() {
            remove_git_hook_section(&hook_path)?;
        }
    }

    // 4. Remove config dir
    if let Some(config_dir) = dirs::config_dir().map(|d| d.join("cwinner")) {
        if config_dir.exists() {
            let _ = std::fs::remove_dir_all(&config_dir)
                .map(|()| println!("✓ Removed {}", config_dir.display()));
        }
    }

    // 5. Remove state dir (includes socket)
    if let Some(state_dir) = dirs::data_local_dir().map(|d| d.join("cwinner")) {
        if state_dir.exists() {
            let _ = std::fs::remove_dir_all(&state_dir)
                .map(|()| println!("✓ Removed {}", state_dir.display()));
        }
    }

    println!("✓ cwinner uninstalled");
    Ok(())
}

pub fn remove_claude_hooks(settings_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(settings_path)?;
    let mut v: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_name, arr) in hooks.iter_mut() {
            if let Some(entries) = arr.as_array_mut() {
                entries
                    .retain(|entry| !entry_has_cwinner(entry) && !entry_has_cwinner_legacy(entry));
            }
        }
    }

    std::fs::write(settings_path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

pub fn remove_git_hook_section(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    if let (Some(start), Some(end)) = (
        content.find(HOOK_MARKER_START),
        content.find(HOOK_MARKER_END),
    ) {
        let end_of_marker = end + HOOK_MARKER_END.len();
        let after = content[end_of_marker..]
            .strip_prefix('\n')
            .unwrap_or(&content[end_of_marker..]);
        let mut remaining = format!("{}{}", &content[..start], after);
        remaining.truncate(remaining.trim_end().len());
        if !remaining.is_empty() {
            remaining.push('\n');
        }

        // If only the shebang remains, delete the file
        let meaningful = remaining.trim();
        if meaningful.is_empty()
            || meaningful == "#!/usr/bin/env bash"
            || meaningful == "#!/bin/bash"
        {
            std::fs::remove_file(path)?;
            println!("✓ Removed {}", path.display());
        } else {
            std::fs::write(path, remaining)?;
            println!("✓ Removed cwinner section from {}", path.display());
        }
    } else if content.contains("cwinner") {
        // Legacy hook without markers — remove entirely
        std::fs::remove_file(path)?;
        println!("✓ Removed legacy hook {}", path.display());
    }
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
        assert_eq!(v["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);
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
        let cwinner_count = hooks.iter().filter(|h| entry_has_cwinner(h)).count();
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

    #[test]
    fn test_hook_chaining_new_file() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        install_git_hook(
            &hook_path,
            "#!/usr/bin/env bash\n# hook content\necho hello\n",
        )
        .unwrap();

        let content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(content.starts_with("#!/usr/bin/env bash\n"));
        assert!(content.contains(HOOK_MARKER_START));
        assert!(content.contains(HOOK_MARKER_END));
        assert!(content.contains("echo hello"));
        // Shebang should not be duplicated inside the marker section
        let marker_section_start = content.find(HOOK_MARKER_START).unwrap();
        let section = &content[marker_section_start..];
        assert!(!section.contains("#!/usr/bin/env bash"));
    }

    #[test]
    fn test_hook_chaining_append_to_existing() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        std::fs::write(&hook_path, "#!/usr/bin/env bash\necho existing\n").unwrap();

        install_git_hook(
            &hook_path,
            "#!/usr/bin/env bash\n# cwinner hook\necho cwinner\n",
        )
        .unwrap();

        let content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(
            content.contains("echo existing"),
            "existing hook content preserved"
        );
        assert!(content.contains(HOOK_MARKER_START));
        assert!(content.contains("echo cwinner"));
        // Existing content should come before cwinner section
        let existing_pos = content.find("echo existing").unwrap();
        let marker_pos = content.find(HOOK_MARKER_START).unwrap();
        assert!(existing_pos < marker_pos);
    }

    #[test]
    fn test_hook_chaining_replace_existing_cwinner() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        // First install
        let existing = format!(
            "#!/usr/bin/env bash\necho existing\n\n{}\n# old cwinner content\n{}\n",
            HOOK_MARKER_START, HOOK_MARKER_END
        );
        std::fs::write(&hook_path, &existing).unwrap();

        install_git_hook(&hook_path, "#!/usr/bin/env bash\n# new cwinner\necho new\n").unwrap();

        let content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(
            content.contains("echo existing"),
            "existing hook content preserved"
        );
        assert!(content.contains("echo new"), "new cwinner content present");
        assert!(
            !content.contains("old cwinner content"),
            "old cwinner content replaced"
        );
        // Markers should appear exactly once
        assert_eq!(content.matches(HOOK_MARKER_START).count(), 1);
        assert_eq!(content.matches(HOOK_MARKER_END).count(), 1);
    }

    #[test]
    fn test_remove_git_hook_section_cleans_markers() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        let content = format!(
            "#!/usr/bin/env bash\necho keep_this\n\n{}\n# cwinner stuff\n{}\n",
            HOOK_MARKER_START, HOOK_MARKER_END
        );
        std::fs::write(&hook_path, content).unwrap();

        remove_git_hook_section(&hook_path).unwrap();

        let remaining = std::fs::read_to_string(&hook_path).unwrap();
        assert!(
            remaining.contains("echo keep_this"),
            "non-cwinner content preserved"
        );
        assert!(!remaining.contains(HOOK_MARKER_START));
        assert!(!remaining.contains("cwinner stuff"));
    }

    #[test]
    fn test_remove_git_hook_section_removes_empty_file() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        let content = format!(
            "#!/usr/bin/env bash\n{}\n# cwinner stuff\n{}\n",
            HOOK_MARKER_START, HOOK_MARKER_END
        );
        std::fs::write(&hook_path, content).unwrap();

        remove_git_hook_section(&hook_path).unwrap();

        assert!(!hook_path.exists(), "empty hook file should be deleted");
    }

    #[test]
    fn test_remove_claude_hooks() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, "{}").unwrap();

        // Add hooks
        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        assert!(
            content.contains("cwinner"),
            "cwinner hooks should be present after add"
        );

        // Remove hooks
        remove_claude_hooks(&settings_path).unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        // All hook arrays should be empty
        for hook_name in &["PostToolUse", "TaskCompleted", "Stop"] {
            let arr = v["hooks"][hook_name].as_array().unwrap();
            assert!(arr.is_empty(), "{hook_name} should be empty after remove");
        }
    }

    #[test]
    fn test_remove_git_hook_section_noop_when_no_cwinner() {
        let dir = tempdir().unwrap();
        let hook_path = dir.path().join("post-commit");
        let content = "#!/usr/bin/env bash\necho other_tool\n";
        std::fs::write(&hook_path, content).unwrap();

        remove_git_hook_section(&hook_path).unwrap();

        let remaining = std::fs::read_to_string(&hook_path).unwrap();
        assert_eq!(remaining, content, "file should be unchanged");
    }

    #[test]
    fn test_remove_claude_hooks_legacy_format() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(
            &settings_path,
            r#"{"hooks":{"PostToolUse":[{"cmd":"/usr/local/bin/cwinner hook post-tool-use"},{"cmd":"other-tool"}]}}"#,
        ).unwrap();

        remove_claude_hooks(&settings_path).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = v["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "only non-cwinner entry should remain");
        assert_eq!(arr[0]["cmd"].as_str().unwrap(), "other-tool");
    }
}
