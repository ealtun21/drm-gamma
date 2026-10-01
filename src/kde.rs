//! KDE Plasma (KWin Wayland) backend: apply an ICC profile to every enabled
//! output via `kscreen-doctor`. KWin owns the gamma LUT and overwrites DRM
//! writes, but it honours an ICC profile's vcgt curve and keeps it across reboots.

use std::path::{Path, PathBuf};
use std::process::Command;

/// True if a KWin Wayland compositor is running (env-independent, so it also
/// works over ssh or under sudo).
pub fn kwin_running() -> bool {
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return false;
    };
    procs.flatten().any(|p| {
        std::fs::read_to_string(p.path().join("comm")).is_ok_and(|c| c.trim() == "kwin_wayland")
    })
}

/// Default profile directory: $XDG_DATA_HOME/icc or ~/.local/share/icc.
pub fn icc_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("icc")
}

/// `program` with the session's Wayland socket and D-Bus, even from ssh/tty.
fn session_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", nix::unistd::getuid())));
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        let socket = std::fs::read_dir(&runtime).ok().and_then(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("wayland-") && !n.ends_with(".lock"))
                .min()
        });
        if let Some(s) = socket {
            cmd.env("WAYLAND_DISPLAY", s);
        }
    }
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        cmd.env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}/bus", runtime.display()),
        );
    }
    cmd.env("XDG_RUNTIME_DIR", runtime);
    cmd
}

fn kscreen_doctor() -> Command {
    session_cmd("kscreen-doctor")
}

/// Turn KWin Night Light off so only drm-gamma's settings tint the screen
/// (KWin computes its tint in the profile's primaries, so saturation would
/// amplify it). Returns true if it was on.
pub fn disable_night_light() -> bool {
    let cfg = [
        "--file",
        "kwinrc",
        "--group",
        "NightColor",
        "--key",
        "Active",
    ];
    let on = session_cmd("kreadconfig6")
        .args(cfg)
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "true");
    if on {
        // --notify makes KWin's config watcher reload it immediately.
        let _ = session_cmd("kwriteconfig6")
            .args(cfg)
            .args(["--notify", "--type", "bool", "false"])
            .status();
    }
    on
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip CSI sequence up to its final letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Enabled output names from `kscreen-doctor -o` text.
fn parse_outputs(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = None;
    for line in strip_ansi(text).lines() {
        if let Some(rest) = line.strip_prefix("Output: ") {
            current = rest.split_whitespace().nth(1).map(str::to_string);
        } else if line.trim() == "enabled" {
            names.extend(current.take());
        }
    }
    names
}

fn outputs() -> Result<Vec<String>, String> {
    let out = kscreen_doctor()
        .arg("-o")
        .output()
        .map_err(|e| format!("kscreen-doctor: {e}"))?;
    let names = parse_outputs(&String::from_utf8_lossy(&out.stdout));
    if names.is_empty() {
        return Err("kscreen-doctor reported no enabled outputs".into());
    }
    Ok(names)
}

fn run(args: Vec<String>) -> Result<(), String> {
    let status = kscreen_doctor()
        .args(&args)
        .status()
        .map_err(|e| format!("kscreen-doctor: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kscreen-doctor failed ({status})"))
    }
}

/// Point every enabled output at `profile`. Returns the output names.
pub fn apply(profile: &Path) -> Result<Vec<String>, String> {
    let names = outputs()?;
    let p = profile.display();
    run(names
        .iter()
        .flat_map(|n| {
            [
                format!("output.{n}.iccprofile.{p}"),
                format!("output.{n}.colorProfileSource.ICC"),
            ]
        })
        .collect())?;
    Ok(names)
}

/// Back to KWin's built-in sRGB handling on every enabled output.
pub fn reset() -> Result<Vec<String>, String> {
    let names = outputs()?;
    run(names
        .iter()
        .map(|n| format!("output.{n}.colorProfileSource.sRGB"))
        .collect())?;
    Ok(names)
}

/// Delete our older auto-named profiles in `dir`, keeping `keep`.
pub fn prune(dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("drm-gamma-") && name.ends_with(".icc") && e.path() != keep {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_outputs() {
        let text = "\x1b[01;32mOutput: \x1b[0;0m1 eDP-1 9b13e0b6\n\tenabled\n\tconnected\n\
                    Output: 2 HDMI-A-1 abcd\n\tdisabled\n\tconnected\n\
                    Output: 3 DP-2 ef01\n\tenabled\n";
        assert_eq!(parse_outputs(text), vec!["eDP-1", "DP-2"]);
    }
}
