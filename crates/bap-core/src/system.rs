//! What machine this is: the distribution, the desktop, which tools exist
//! and where things live. Everything here is read once and cheap.

use crate::model::SystemInfo;
use std::path::{Path, PathBuf};

/// Read `/etc/os-release` and the session environment.
pub fn detect() -> SystemInfo {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    from_os_release(&text)
}

/// The pure half of [`detect`], so it can be tested on any machine.
pub fn from_os_release(text: &str) -> SystemInfo {
    let mut id = String::new();
    let mut like = Vec::new();
    let mut pretty = String::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_matches('"');
        match k.trim() {
            "ID" => id = v.to_string(),
            "ID_LIKE" => like = v.split_whitespace().map(str::to_string).collect(),
            "PRETTY_NAME" => pretty = v.to_string(),
            _ => {}
        }
    }
    if id.is_empty() {
        id = "linux".to_string();
    }
    if pretty.is_empty() {
        pretty = id.clone();
    }
    SystemInfo {
        distro_id: id,
        distro_like: like,
        pretty_name: pretty,
        arch: std::env::consts::ARCH.to_string(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .filter(|s| !s.is_empty()),
        session: std::env::var("XDG_SESSION_TYPE")
            .ok()
            .filter(|s| !s.is_empty()),
    }
}

/// The first directory on `PATH` holding an executable of that name.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Run a program and return its stdout as text, or the sentence that
/// explains why not. For read-only queries of tools like `flatpak` and
/// `fwupdmgr`; never for anything that changes the machine (that is a Step).
pub fn run(program: &str, args: &[&str]) -> crate::Result<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .map_err(|e| crate::Error::new(format!("could not run {program}: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().next().unwrap_or("").trim();
        return Err(crate::Error::new(format!(
            "{program} {} failed{}",
            args.first().copied().unwrap_or(""),
            if first.is_empty() {
                String::new()
            } else {
                format!(": {first}")
            }
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// An NVIDIA GPU is present, judged from sysfs vendor ids (0x10de) so no
/// tool needs to be installed. Used for the one WebKit workaround in
/// `bap-store`.
pub fn has_nvidia() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/bus/pci/devices") else {
        return false;
    };
    entries.flatten().any(|e| {
        std::fs::read_to_string(e.path().join("vendor"))
            .map(|v| v.trim() == "0x10de")
            .unwrap_or(false)
    })
}

/// Where BAP Store keeps its own files.
pub struct Dirs {
    pub cache: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Dirs {
    pub fn new() -> Dirs {
        let base = directories::ProjectDirs::from("io.github", "spillebulle", "bap-store");
        match base {
            Some(d) => Dirs {
                cache: d.cache_dir().to_path_buf(),
                config: d.config_dir().to_path_buf(),
                data: d.data_dir().to_path_buf(),
            },
            None => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                Dirs {
                    cache: PathBuf::from(&home).join(".cache/bap-store"),
                    config: PathBuf::from(&home).join(".config/bap-store"),
                    data: PathBuf::from(&home).join(".local/share/bap-store"),
                }
            }
        }
    }
}

impl Default for Dirs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cachyos_is_arch_like() {
        let s = from_os_release(
            "NAME=\"CachyOS Linux\"\nPRETTY_NAME=\"CachyOS\"\nID=cachyos\nID_LIKE=arch\n",
        );
        assert_eq!(s.distro_id, "cachyos");
        assert!(s.is_arch_like());
        assert!(!s.is_debian_like());
        assert_eq!(s.pretty_name, "CachyOS");
    }

    #[test]
    fn ubuntu_is_debian_like_and_mint_is_too() {
        let u = from_os_release("ID=ubuntu\nID_LIKE=debian\n");
        assert!(u.is_debian_like());
        let m = from_os_release("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n");
        assert!(m.is_debian_like());
    }

    #[test]
    fn an_empty_file_is_still_a_system() {
        let s = from_os_release("");
        assert_eq!(s.distro_id, "linux");
        assert_eq!(s.pretty_name, "linux");
    }
}
