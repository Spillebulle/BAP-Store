//! pacman's databases, read directly.
//!
//! The local database is a directory per installed package under
//! `/var/lib/pacman/local/<name>-<version>/` holding a `desc` file. A sync
//! database (`/var/lib/pacman/sync/<repo>.db`) is a tar archive, gzip or
//! zstd compressed, of the same `<name>-<version>/desc` layout. `desc` is a
//! sequence of `%KEY%` lines each followed by its values, blank-line
//! separated. That is the whole format, and reading it here rather than
//! through libalpm is what lets one binary run on every distribution.

use crate::{Error, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// One `desc` file, keys as written (`NAME`, `VERSION`, `DESC`, …).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Desc {
    pub fields: HashMap<String, Vec<String>>,
}

impl Desc {
    pub fn parse(text: &str) -> Desc {
        let mut fields: HashMap<String, Vec<String>> = HashMap::new();
        let mut key: Option<String> = None;
        for line in text.lines() {
            if line.len() >= 2 && line.starts_with('%') && line.ends_with('%') {
                key = Some(line[1..line.len() - 1].to_string());
                fields.entry(key.clone().unwrap()).or_default();
            } else if line.is_empty() {
                continue;
            } else if let Some(k) = &key {
                fields.get_mut(k).unwrap().push(line.to_string());
            }
        }
        Desc { fields }
    }

    pub fn first(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(|v| v.first()).map(String::as_str)
    }

    pub fn all(&self, key: &str) -> &[String] {
        self.fields.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn u64(&self, key: &str) -> Option<u64> {
        self.first(key).and_then(|s| s.parse().ok())
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        self.first(key).and_then(|s| s.parse().ok())
    }

    pub fn name(&self) -> &str {
        self.first("NAME").unwrap_or("")
    }

    pub fn version(&self) -> &str {
        self.first("VERSION").unwrap_or("")
    }
}

/// The installed packages, keyed by name.
#[derive(Clone, Debug, Default)]
pub struct LocalDb {
    pub packages: HashMap<String, Desc>,
}

impl LocalDb {
    pub const DEFAULT_PATH: &str = "/var/lib/pacman/local";

    pub fn load(dir: &Path) -> Result<LocalDb> {
        let mut packages = HashMap::new();
        let entries = std::fs::read_dir(dir)
            .map_err(|e| Error::new(format!("could not read {}: {e}", dir.display())))?;
        for entry in entries.flatten() {
            let path = entry.path().join("desc");
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let desc = Desc::parse(&text);
            if !desc.name().is_empty() {
                packages.insert(desc.name().to_string(), desc);
            }
        }
        Ok(LocalDb { packages })
    }

    pub fn get(&self, name: &str) -> Option<&Desc> {
        self.packages.get(name)
    }
}

/// One repository's sync database.
#[derive(Clone, Debug, Default)]
pub struct SyncDb {
    pub repo: String,
    pub packages: HashMap<String, Desc>,
}

impl SyncDb {
    /// Read `<repo>.db`, whichever compression it uses.
    pub fn load(repo: &str, path: &Path) -> Result<SyncDb> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::new(format!("could not read {}: {e}", path.display())))?;
        Self::from_bytes(repo, &bytes)
    }

    pub fn from_bytes(repo: &str, bytes: &[u8]) -> Result<SyncDb> {
        let plain = decompress(bytes)?;
        let mut archive = tar::Archive::new(plain.as_slice());
        let mut packages = HashMap::new();
        let entries = archive
            .entries()
            .map_err(|e| Error::new(format!("{repo}.db is not a tar archive: {e}")))?;
        for entry in entries {
            let mut entry = entry.map_err(|e| Error::new(format!("{repo}.db: {e}")))?;
            let path = entry.path().map(|p| p.to_path_buf()).unwrap_or_default();
            if path.file_name().and_then(|f| f.to_str()) != Some("desc") {
                continue;
            }
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| Error::new(format!("{repo}.db: {e}")))?;
            let desc = Desc::parse(&text);
            if !desc.name().is_empty() {
                packages.insert(desc.name().to_string(), desc);
            }
        }
        Ok(SyncDb {
            repo: repo.to_string(),
            packages,
        })
    }

    pub fn get(&self, name: &str) -> Option<&Desc> {
        self.packages.get(name)
    }
}

/// gzip, zstd or already plain, judged by the magic bytes.
fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| Error::new(format!("gzip: {e}")))?;
        Ok(out)
    } else if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        let mut out = Vec::new();
        ruzstd::decoding::StreamingDecoder::new(bytes)
            .map_err(|e| Error::new(format!("zstd: {e}")))?
            .read_to_end(&mut out)
            .map_err(|e| Error::new(format!("zstd: {e}")))?;
        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

/// The repositories `pacman.conf` names, in order, with their database
/// paths. Includes files pulled in with `Include =` only as far as their
/// section names go: mirrors are not needed to read a database.
pub fn repos(conf: &str, sync_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for line in conf.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim();
            if name != "options" {
                out.push((name.to_string(), sync_dir.join(format!("{name}.db"))));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAM: &str = "%FILENAME%\nsteam-1.0.0.87-3-x86_64.pkg.tar.zst\n\n%NAME%\nsteam\n\n%BASE%\nsteam\n\n%VERSION%\n1.0.0.87-3\n\n%DESC%\nValve's digital software delivery system\n\n%CSIZE%\n20371240\n\n%ISIZE%\n20475439\n\n%URL%\nhttps://steampowered.com/\n\n%LICENSE%\nLicenseRef-steam-subscriber-agreement\n\n%ARCH%\nx86_64\n\n%BUILDDATE%\n1785004799\n\n%DEPENDS%\nbash\ncoreutils\n";

    #[test]
    fn a_desc_file_reads_back() {
        let d = Desc::parse(STEAM);
        assert_eq!(d.name(), "steam");
        assert_eq!(d.version(), "1.0.0.87-3");
        assert_eq!(d.u64("CSIZE"), Some(20_371_240));
        assert_eq!(d.all("DEPENDS"), ["bash", "coreutils"]);
        assert_eq!(d.first("MISSING"), None);
        assert!(d.all("MISSING").is_empty());
    }

    #[test]
    fn repos_come_out_in_pacman_conf_order() {
        let conf = "[options]\nHoldPkg = pacman\n\n[cachyos-v3]\nInclude = /etc/pacman.d/x\n[core]\nInclude = /etc/pacman.d/mirrorlist\n#[testing]\n[extra]\nInclude = /etc/pacman.d/mirrorlist\n";
        let r = repos(conf, Path::new("/var/lib/pacman/sync"));
        let names: Vec<&str> = r.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["cachyos-v3", "core", "extra"]);
        assert_eq!(r[1].1, PathBuf::from("/var/lib/pacman/sync/core.db"));
    }

    #[test]
    fn a_plain_tar_is_a_database_too() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(STEAM.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "steam-1.0.0.87-3/desc", STEAM.as_bytes())
            .unwrap();
        let bytes = builder.into_inner().unwrap();
        let db = SyncDb::from_bytes("multilib", &bytes).unwrap();
        assert_eq!(db.get("steam").unwrap().version(), "1.0.0.87-3");
    }

    #[test]
    #[ignore = "reads this machine's pacman databases"]
    fn live_this_machine_s_databases_read() {
        let local = LocalDb::load(Path::new(LocalDb::DEFAULT_PATH)).unwrap();
        assert!(local.packages.len() > 10);
        let conf = std::fs::read_to_string("/etc/pacman.conf").unwrap();
        for (repo, path) in repos(&conf, Path::new("/var/lib/pacman/sync")) {
            if path.exists() {
                let db = SyncDb::load(&repo, &path).unwrap();
                assert!(!db.packages.is_empty(), "{repo} is empty");
            }
        }
    }
}
