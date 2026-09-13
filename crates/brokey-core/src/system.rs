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
        .map_err(|e| crate::Error::new(format!("Could not run {program}: {e}.")))?;
    if !out.status.success() {
        return Err(crate::Error::new(run_failure(
            program,
            args.first().copied().unwrap_or(""),
            &String::from_utf8_lossy(&out.stderr),
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The sentence for a tool that exited with an error: the program, its
/// verb and the first line it printed. A full sentence, because three
/// callers show it to the user as it is.
fn run_failure(program: &str, verb: &str, stderr: &str) -> String {
    let first = stderr.lines().next().unwrap_or("").trim();
    let verb = if verb.is_empty() {
        String::new()
    } else {
        format!(" {verb}")
    };
    if first.is_empty() {
        format!("{program}{verb} failed.")
    } else {
        format!("{program}{verb} failed: {}.", first.trim_end_matches('.'))
    }
}

/// An NVIDIA GPU is present, judged from sysfs vendor ids (0x10de) so no
/// tool needs to be installed. Used for the one WebKit workaround in
/// `brokey`.
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

/// Where Brokey keeps its own files.
pub struct Dirs {
    /// Always `$HOME/.cache/brokey`, never `XDG_CACHE_HOME`: see
    /// [`cache_dir_for`].
    pub cache: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
}

/// The store's cache directory for a home: `<home>/.cache/brokey`.
///
/// This is fixed rather than read from `XDG_CACHE_HOME` because the helper
/// installs a downloaded package file only from the directories in
/// `transaction::allow::Allowed::for_home`, which names exactly this path
/// under the invoking user's passwd home. The helper runs under pkexec with
/// the user's environment scrubbed, so it cannot follow `XDG_CACHE_HOME`;
/// if the store did, a download would land where the helper refuses to
/// install from.
pub fn cache_dir_for(home: &Path) -> PathBuf {
    home.join(".cache").join("brokey")
}

impl Dirs {
    pub fn new() -> Dirs {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let cache = cache_dir_for(&home);
        let base = directories::ProjectDirs::from("io.github", "spillebulle", "brokey");
        match base {
            Some(d) => Dirs {
                cache,
                config: d.config_dir().to_path_buf(),
                data: d.data_dir().to_path_buf(),
            },
            None => Dirs {
                cache,
                config: home.join(".config/brokey"),
                data: home.join(".local/share/brokey"),
            },
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

    #[test]
    fn the_cache_is_where_the_helper_allows_package_files_from() {
        let home = Path::new("/home/me");
        assert_eq!(cache_dir_for(home), PathBuf::from("/home/me/.cache/brokey"));
        let allowed = crate::transaction::Allowed::for_home(Some(home));
        assert!(
            allowed.package_dirs.contains(&cache_dir_for(home)),
            "the store's cache and the helper's list name the same directory"
        );
        assert_eq!(
            cache_dir_for(Path::new("/")),
            PathBuf::from("/.cache/brokey")
        );
    }

    #[test]
    fn run_failures_are_sentences() {
        assert_eq!(
            run_failure(
                "fwupdmgr",
                "get-devices",
                "error: failed to connect to daemon\nmore\n"
            ),
            "fwupdmgr get-devices failed: error: failed to connect to daemon."
        );
        assert_eq!(
            run_failure("flatpak", "remotes", "error: Unable to load summary.\n"),
            "flatpak remotes failed: error: Unable to load summary.",
            "one full stop, not two"
        );
        assert_eq!(run_failure("chwd", "-i", "\n"), "chwd -i failed.");
        assert_eq!(run_failure("chwd", "", ""), "chwd failed.");
    }
}
