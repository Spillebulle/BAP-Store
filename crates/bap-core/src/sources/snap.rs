//! The Snap Store, through snapd's REST API on `/run/snapd.socket`.
//!
//! snapd is the only way in. The public store API at api.snapcraft.io can
//! search without it, but a result the machine cannot install is a lie, so
//! when snapd is absent this source says why and does nothing else. The API
//! is HTTP/1.1 over a unix socket, and rather than pull in a crate for that
//! a minimal client lives here: one request line, four headers, a status
//! line, headers, and a body by content-length or chunked. That is all snapd
//! ever sends, and it makes the parser testable from a byte string.
//!
//! Endpoints used, and the fields of each answer this module relies on. The
//! shape is snapd's `client.Snap`, tags as written in `client/packages.go`:
//!
//! - `GET /v2/system-info`: `version`, `locations.snap-mount-dir`.
//! - `GET /v2/snaps` and `GET /v2/snaps/<name>`, the installed snaps:
//!   `name`, `title`, `summary`, `description`, `version`, `revision` (a
//!   JSON string, `"6338"`, or `"x1"` for a sideload), `channel`,
//!   `tracking-channel`, `publisher{display-name, validation}`, `icon`
//!   (a `/v2/icons/<name>/icon` path served by snapd itself, so it is
//!   resolved to the file under the mount directory), `install-date`
//!   (RFC 3339), `installed-size`, `confinement`, `type` (`app`, `base`,
//!   `os`, `snapd`, `kernel`, `gadget`), `apps[]{desktop-file, common-id}`,
//!   `common-ids`, `license`, `store-url`, `website`, `media[]`.
//! - `GET /v2/find?q=<term>` and `GET /v2/find?name=<name>`, the store: the
//!   same plus `download-size`, `media[]{type, url, width, height}` with
//!   types `icon`, `screenshot`, `banner` and `video`, `categories[]{name}`.
//! - `GET /v2/find?select=refresh`: the same shape, one entry per installed
//!   snap with a newer revision in its channel, carrying the new version.
//!
//! Every answer is `{"type":"sync","status-code":200,"result":...}`; an
//! error is `{"type":"error","status-code":N,"result":{"message","kind"}}`.
//! Go's HTTP server sends small bodies with `Content-Length` and larger
//! ones chunked, so both are needed in practice, not just for completeness.

use crate::model::*;
use crate::{Error, Op, Query, Result, Source};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where snapd listens on every distribution that ships it.
pub const SOCKET_PATH: &str = "/run/snapd.socket";

/// Where snaps are mounted: `/snap` on Ubuntu and Debian, the second on
/// Arch and Fedora, which do not put a directory in `/`.
const MOUNT_DIRS: [&str; 2] = ["/snap", "/var/lib/snapd/snap"];

/// What of snapd is on this machine, as far as the status line needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// The socket exists: snapd is installed and, unless it has crashed, listening.
    Socket,
    /// `snap` is on `PATH` but the socket is not: installed and not started.
    ProgramOnly,
    /// Neither. snapd is not installed.
    Absent,
}

/// One HTTP answer from snapd, whatever its status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// How this source reaches snapd. The real one talks to the socket; tests
/// hand in canned answers, which is what lets the whole source be tested on
/// a machine without snapd.
pub trait Transport: Send + Sync {
    fn presence(&self) -> Presence;

    /// GET an API path such as `/v2/snaps`. Any HTTP status is an `Ok`
    /// response; `Err` means snapd could not be reached at all.
    fn get(&self, path: &str) -> Result<Response>;
}

/// The real transport: HTTP/1.1 over the unix socket.
pub struct SocketTransport {
    socket: PathBuf,
    timeout: Duration,
}

impl SocketTransport {
    pub fn new() -> SocketTransport {
        SocketTransport::at(Path::new(SOCKET_PATH))
    }

    /// A transport on another socket, for tests that run their own server.
    pub fn at(socket: &Path) -> SocketTransport {
        SocketTransport {
            socket: socket.to_path_buf(),
            // A find goes out to the store and can take seconds; the same
            // ceiling the HTTP client uses, so a hung snapd is reported in
            // the same time as a hung web service.
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for SocketTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for SocketTransport {
    fn presence(&self) -> Presence {
        if self.socket.exists() {
            Presence::Socket
        } else if crate::system::which("snap").is_some() {
            Presence::ProgramOnly
        } else {
            Presence::Absent
        }
    }

    fn get(&self, path: &str) -> Result<Response> {
        let mut stream = UnixStream::connect(&self.socket)
            .map_err(|e| snap_error(connect_failure(&self.socket, &e)))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|_| stream.set_write_timeout(Some(self.timeout)))
            .map_err(|e| snap_error(format!("Could not set a timeout on snapd's socket: {e}.")))?;
        stream.write_all(request(path).as_bytes()).map_err(|e| {
            snap_error(format!(
                "snapd stopped listening while {path} was being sent: {e}. Try again."
            ))
        })?;
        let mut reader = BufReader::new(stream);
        read_response(&mut reader)
    }
}

/// The request for one API path. `Connection: close` so the body ends at
/// EOF even if a proxy strips the length; `Host` because Go's server
/// answers 400 without one.
pub fn request(path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nUser-Agent: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        crate::http::USER_AGENT
    )
}

/// Parse one HTTP/1.1 response held entirely in memory.
pub fn parse_response(bytes: &[u8]) -> Result<Response> {
    let mut cursor = bytes;
    read_response(&mut cursor)
}

/// Read one HTTP/1.1 response: status line, headers, then the body by
/// `Content-Length`, by chunked transfer encoding, or to EOF when neither
/// is given.
pub fn read_response<R: BufRead>(r: &mut R) -> Result<Response> {
    let mut line = String::new();
    read_line(r, &mut line)?;
    let status = parse_status_line(&line)?;
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    loop {
        line.clear();
        read_line(r, &mut line)?;
        let l = line.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            break;
        }
        let Some((name, value)) = l.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                let n = value.trim().parse::<usize>().map_err(|_| {
                    snap_error(format!(
                        "snapd sent a Content-Length that is not a number: {:?}.",
                        value.trim()
                    ))
                })?;
                content_length = Some(n);
            }
            "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
            _ => {}
        }
    }
    let body = if chunked {
        read_chunked(r)?
    } else if let Some(n) = content_length {
        let mut body = vec![0; n];
        r.read_exact(&mut body).map_err(|_| closed_early())?;
        body
    } else {
        let mut body = Vec::new();
        r.read_to_end(&mut body)
            .map_err(|e| snap_error(format!("snapd stopped answering: {e}. Try again.")))?;
        body
    };
    Ok(Response { status, body })
}

fn parse_status_line(line: &str) -> Result<u16> {
    let mut parts = line.split_whitespace();
    let version = parts.next().unwrap_or("");
    let code = parts.next().and_then(|c| c.parse::<u16>().ok());
    match code {
        Some(code) if version.starts_with("HTTP/") => Ok(code),
        _ => Err(snap_error(format!(
            "snapd answered with something that is not HTTP: {:?}.",
            line.trim_end()
        ))),
    }
}

fn read_chunked<R: BufRead>(r: &mut R) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        read_line(r, &mut line)?;
        // A chunk size may carry extensions after a semicolon; nobody sends
        // them but the grammar allows it.
        let size_text = line.trim().split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| {
            snap_error(format!(
                "snapd sent a chunk size that is not a number: {size_text:?}."
            ))
        })?;
        if size == 0 {
            // Trailers, if any, run to an empty line. EOF there is fine:
            // the answer is already complete.
            loop {
                line.clear();
                let n = r.read_line(&mut line).map_err(|_| closed_early())?;
                if n == 0 || line.trim_end_matches(['\r', '\n']).is_empty() {
                    break;
                }
            }
            return Ok(body);
        }
        let start = body.len();
        body.resize(start + size, 0);
        r.read_exact(&mut body[start..])
            .map_err(|_| closed_early())?;
        line.clear();
        read_line(r, &mut line)?;
    }
}

fn read_line<R: BufRead>(r: &mut R, buf: &mut String) -> Result<()> {
    match r.read_line(buf) {
        Ok(0) => Err(closed_early()),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Err(snap_error(
            "snapd sent a header that is not text.".to_string(),
        )),
        Err(_) => Err(closed_early()),
    }
}

fn closed_early() -> Error {
    snap_error("snapd closed the connection before its answer was complete. Try again.".to_string())
}

fn connect_failure(socket: &Path, e: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    match e.kind() {
        ConnectionRefused => {
            "snapd is not running. Start it with systemctl enable --now snapd.socket.".to_string()
        }
        NotFound => format!(
            "snapd's socket {} is missing. Install snapd and start it with systemctl enable --now snapd.socket.",
            socket.display()
        ),
        PermissionDenied => format!(
            "snapd's socket {} does not allow this user to connect. Check its permissions.",
            socket.display()
        ),
        _ => format!("Could not connect to snapd at {}: {e}.", socket.display()),
    }
}

fn snap_error(message: String) -> Error {
    Error::from_source(SourceKind::Snap, message)
}

/// Percent-encode a query value or a path segment. Snap names are
/// `[a-z0-9-]`, so this only matters for search text.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// RFC 3339 (`2026-08-30T14:22:07.517389211+02:00`) to unix seconds. Go
/// writes nanoseconds and a numeric offset or `Z`; nothing else is
/// accepted, and a date that does not parse is simply unknown.
pub fn rfc3339_to_unix(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    let second = num(17, 19)?;
    let separators_ok = b[4] == b'-'
        && b[7] == b'-'
        && matches!(b[10], b'T' | b't' | b' ')
        && b[13] == b':'
        && b[16] == b':';
    if !separators_ok
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    let offset = match &s[i..] {
        "" | "Z" | "z" => 0,
        zone => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hh = zone.get(1..3)?.parse::<i64>().ok()?;
            let mm = zone.get(4..6)?.parse::<i64>().ok()?;
            if zone.as_bytes().get(3) != Some(&b':') || zone.len() != 6 {
                return None;
            }
            sign * (hh * 3600 + mm * 60)
        }
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// algorithm), so no calendar crate is needed for one field.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// snapd's envelope around every answer.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "status-code")]
    status_code: u16,
    result: serde_json::Value,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ApiError {
    message: String,
    kind: String,
}

/// `GET /v2/system-info`, the two fields this module reads.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct SnapdInfo {
    pub version: String,
    pub locations: Locations,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Locations {
    pub snap_mount_dir: String,
}

/// One snap as snapd describes it, installed or in the store. Fields snapd
/// omits come out empty rather than `None` so the mapping reads plainly;
/// `text` turns empty back into `None` for the model.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct SnapInfo {
    pub id: String,
    pub name: String,
    pub title: String,
    pub summary: String,
    pub description: String,
    pub version: String,
    #[serde(deserialize_with = "revision_text")]
    pub revision: String,
    pub channel: String,
    pub tracking_channel: String,
    pub publisher: Option<Publisher>,
    pub developer: String,
    pub icon: String,
    pub install_date: String,
    pub installed_size: i64,
    pub download_size: i64,
    pub confinement: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub base: String,
    pub apps: Vec<AppInfo>,
    pub common_ids: Vec<String>,
    pub license: String,
    pub store_url: String,
    pub website: String,
    pub media: Vec<Media>,
    pub categories: Vec<Category>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Publisher {
    pub id: String,
    pub username: String,
    pub display_name: String,
    /// `verified`, `starred` or `unproven`.
    pub validation: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AppInfo {
    pub name: String,
    pub desktop_file: String,
    pub common_id: String,
    pub daemon: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Media {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Category {
    pub name: String,
    pub featured: bool,
}

/// snapd writes revisions as strings (`"6338"`, `"x1"`). A number is
/// accepted too because it costs nothing and a stricter reader would turn
/// one unexpected answer into an empty installed list.
fn revision_text<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Revision {
        Text(String),
        Number(i64),
    }
    Ok(match Option::<Revision>::deserialize(d)? {
        Some(Revision::Text(s)) => s,
        Some(Revision::Number(n)) => n.to_string(),
        None => String::new(),
    })
}

fn text(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn size(n: i64) -> Option<u64> {
    (n > 0).then_some(n as u64)
}

/// A channel as the store writes it, `track/risk`. Find results name the
/// risk alone (`stable`) while installed snaps carry the full form; one
/// spelling keeps the badge and the Channel fact consistent.
fn full_channel(channel: &str) -> Option<String> {
    let channel = channel.trim();
    if channel.is_empty() {
        None
    } else if channel.contains('/') {
        Some(channel.to_string())
    } else {
        Some(format!("latest/{channel}"))
    }
}

fn has_icon(info: &SnapInfo) -> bool {
    !info.icon.is_empty()
        || info
            .media
            .iter()
            .any(|m| m.kind == "icon" && !m.url.is_empty())
}

/// An application is a snap of type `app` with a picture or a desktop file;
/// a command-line tool has neither. Treating any snap with an `apps[]` entry
/// as an application was rejected: every snap that ships a command has one,
/// so `hello` would be drawn as an application with a letter for an icon.
/// Bases, `core` and snapd itself are runtimes; kernels and gadgets are
/// packages, which a desktop never sees.
pub fn kind_of(info: &SnapInfo, local: Option<&SnapInfo>) -> PackageKind {
    match info.kind.as_str() {
        "app" => {
            let desktop = local
                .into_iter()
                .chain(std::iter::once(info))
                .flat_map(|s| s.apps.iter())
                .any(|a| !a.desktop_file.is_empty());
            let icon = has_icon(info) || local.is_some_and(has_icon);
            if desktop || icon {
                PackageKind::App
            } else {
                PackageKind::Package
            }
        }
        "base" | "os" | "snapd" => PackageKind::Runtime,
        _ => PackageKind::Package,
    }
}

/// The icon: the store's URL where snapd knows it, else the snap's own
/// icon file under the mount directory, which is what the `/v2/icons/…`
/// path snapd advertises serves and the page cannot fetch over a socket.
fn icon_of(info: &SnapInfo, local: Option<&SnapInfo>, mount_dirs: &[PathBuf]) -> Option<Picture> {
    let from_media = |s: &SnapInfo| {
        s.media
            .iter()
            .find(|m| m.kind == "icon" && !m.url.is_empty())
            .map(|m| Picture::Url(m.url.clone()))
    };
    let from_field = |s: &SnapInfo| {
        if s.icon.starts_with("http://") || s.icon.starts_with("https://") {
            Some(Picture::Url(s.icon.clone()))
        } else if s.icon.starts_with("/v2/icons/") {
            mount_dirs
                .iter()
                .flat_map(|dir| {
                    ["png", "svg", "jpg"].map(|ext| {
                        dir.join(&s.name)
                            .join("current/meta/gui")
                            .join(format!("icon.{ext}"))
                    })
                })
                .find(|p| p.is_file())
                .map(Picture::File)
        } else {
            None
        }
    };
    from_media(info)
        .or_else(|| local.and_then(from_media))
        .or_else(|| from_field(info))
        .or_else(|| local.and_then(from_field))
}

fn screenshots_of(info: &SnapInfo) -> Vec<Screenshot> {
    let shot = |m: &Media| Screenshot {
        image: Picture::Url(m.url.clone()),
        thumbnail: None,
        caption: None,
        width: m.width.filter(|w| *w > 0),
        height: m.height.filter(|h| *h > 0),
    };
    // The banner is the store's own hero picture, so it leads the rail and
    // the detail page's backdrop takes it.
    let banner = info
        .media
        .iter()
        .filter(|m| m.kind == "banner" && !m.url.is_empty())
        .map(shot);
    let shots = info
        .media
        .iter()
        .filter(|m| m.kind == "screenshot" && !m.url.is_empty())
        .map(shot);
    banner.chain(shots).collect()
}

/// Build the model's package from what snapd said. `info` is the record
/// the metadata comes from (the store's where it is known, else the
/// installed one); `local` is the installed record when there is one.
pub fn to_package(info: &SnapInfo, local: Option<&SnapInfo>, mount_dirs: &[PathBuf]) -> Package {
    let name = text(&info.title)
        .or_else(|| local.and_then(|l| text(&l.title)))
        .unwrap_or_else(|| info.name.clone());
    let mut p = Package::new(SourceKind::Snap, info.name.clone(), name);
    p.kind = kind_of(info, local);
    p.summary = text(&info.summary).or_else(|| local.and_then(|l| text(&l.summary)));
    p.description = text(&info.description).or_else(|| local.and_then(|l| text(&l.description)));
    p.version = text(&info.version);
    p.installed = local.is_some();
    p.installed_version = local.and_then(|l| text(&l.version));
    let channel_source = local.unwrap_or(info);
    let channel = full_channel(&channel_source.tracking_channel)
        .or_else(|| full_channel(&channel_source.channel));
    p.repo = channel.clone();
    p.licence = text(&info.license).or_else(|| local.and_then(|l| text(&l.license)));
    p.homepage = text(&info.store_url).or_else(|| local.and_then(|l| text(&l.store_url)));
    let publisher = info
        .publisher
        .as_ref()
        .or(local.and_then(|l| l.publisher.as_ref()));
    p.developer = publisher
        .and_then(|pb| text(&pb.display_name).or_else(|| text(&pb.username)))
        .or_else(|| text(&info.developer));
    p.updated = local.and_then(|l| rfc3339_to_unix(&l.install_date));
    p.download_size = size(info.download_size);
    p.installed_size = local
        .and_then(|l| size(l.installed_size))
        .or_else(|| size(info.installed_size));
    p.icon = icon_of(info, local, mount_dirs);
    p.screenshots = if info.media.is_empty() {
        local.map(screenshots_of).unwrap_or_default()
    } else {
        screenshots_of(info)
    };
    p.categories = info
        .categories
        .iter()
        .filter_map(|c| text(&c.name))
        .collect();
    p.appstream_id = info
        .common_ids
        .iter()
        .chain(local.into_iter().flat_map(|l| l.common_ids.iter()))
        .chain(
            local
                .into_iter()
                .flat_map(|l| l.apps.iter().map(|a| &a.common_id)),
        )
        .find_map(|id| text(id));
    // A classic snap has no sandbox at all. Saying "sandboxed" because it is
    // a snap was rejected: the badge would then say the opposite of the
    // Confinement fact beside it.
    let confinement = text(&info.confinement).or_else(|| local.and_then(|l| text(&l.confinement)));
    p.sandboxed = confinement.as_deref() != Some("classic");
    if let Some(pb) = publisher
        && let Some(mut who) = text(&pb.display_name).or_else(|| text(&pb.username))
    {
        if pb.validation == "verified" {
            who.push_str(" (verified)");
        }
        p.facts.push(("Publisher".to_string(), who));
    }
    if let Some(channel) = channel {
        p.facts.push(("Channel".to_string(), channel));
    }
    if let Some(revision) = local
        .and_then(|l| text(&l.revision))
        .or_else(|| text(&info.revision))
    {
        p.facts.push(("Revision".to_string(), revision));
    }
    if let Some(confinement) = confinement {
        p.facts.push(("Confinement".to_string(), confinement));
    }
    if let Some(store) = &p.homepage {
        p.facts.push(("Store page".to_string(), store.clone()));
    }
    p
}

/// The source.
pub struct Snap {
    transport: Box<dyn Transport>,
    arch_like: bool,
    /// Where to look for an installed snap's icon file; snapd's answer to
    /// `/v2/system-info` goes to the front once it has been read.
    mount_dirs: Mutex<Vec<PathBuf>>,
    /// Whether snapd has been asked for that directory. A guess from the
    /// list order was rejected: on Ubuntu the answer is the first guess, and
    /// the question would then be asked on every call.
    mount_dir_known: AtomicBool,
    /// Confinement by snap name, remembered from every answer, so a plan can
    /// add `--classic` without a round trip. `plan` receives only a
    /// reference, never the package with its Confinement fact.
    confinement: Mutex<HashMap<String, String>>,
}

impl Snap {
    /// The HTTP client every source is handed is not used here: the public
    /// store API could search when snapd is absent, but a result that cannot
    /// be installed is a lie, so that path was rejected and the status says
    /// why instead.
    pub fn new(system: &SystemInfo, _client: Arc<crate::http::Client>) -> Snap {
        Snap::with_transport(system, Box::new(SocketTransport::new()))
    }

    pub fn with_transport(system: &SystemInfo, transport: Box<dyn Transport>) -> Snap {
        Snap {
            transport,
            arch_like: system.is_arch_like(),
            mount_dirs: Mutex::new(MOUNT_DIRS.iter().map(PathBuf::from).collect()),
            mount_dir_known: AtomicBool::new(false),
            confinement: Mutex::new(HashMap::new()),
        }
    }

    /// Look for installed snaps' icon files here instead of the system
    /// directories, and do not ask snapd for its own.
    pub fn with_mount_dirs(self, dirs: Vec<PathBuf>) -> Snap {
        *self.mount_dirs.lock().unwrap_or_else(|e| e.into_inner()) = dirs;
        self.mount_dir_known.store(true, Ordering::Relaxed);
        self
    }

    fn mount_dirs(&self) -> Vec<PathBuf> {
        self.mount_dirs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn envelope(&self, path: &str) -> Result<Envelope> {
        let resp = self.transport.get(path)?;
        serde_json::from_slice::<Envelope>(&resp.body).map_err(|e| {
            if resp.status >= 400 {
                snap_error(format!("snapd answered {} for {path}.", resp.status))
            } else {
                snap_error(format!(
                    "snapd sent something that was not the expected JSON for {path}: {e}."
                ))
            }
        })
    }

    fn call<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        decode(self.envelope(path)?, path)
    }

    fn system_info(&self) -> Result<SnapdInfo> {
        let info: SnapdInfo = self.call("/v2/system-info")?;
        if let Some(dir) = text(&info.locations.snap_mount_dir) {
            let dir = PathBuf::from(dir);
            let mut dirs = self.mount_dirs.lock().unwrap_or_else(|e| e.into_inner());
            dirs.retain(|d| *d != dir);
            dirs.insert(0, dir);
        }
        self.mount_dir_known.store(true, Ordering::Relaxed);
        Ok(info)
    }

    /// The installed snaps by name.
    fn local_map(&self) -> Result<HashMap<String, SnapInfo>> {
        let snaps: Vec<SnapInfo> = self.call("/v2/snaps")?;
        Ok(snaps.into_iter().map(|s| (s.name.clone(), s)).collect())
    }

    /// One installed snap, or `None` when it is not installed.
    fn local_one(&self, name: &str) -> Result<Option<SnapInfo>> {
        let env = self.envelope(&format!("/v2/snaps/{}", encode(name)))?;
        if env.status_code == 404 {
            return Ok(None);
        }
        decode(env, name).map(Some)
    }

    /// One snap from the store, or `None` when the store has no such name.
    fn find_one(&self, name: &str) -> Result<Option<SnapInfo>> {
        let path = format!("/v2/find?name={}", encode(name));
        let env = self.envelope(&path)?;
        if env.status_code == 404 {
            return Ok(None);
        }
        let found: Vec<SnapInfo> = decode(env, &path)?;
        Ok(found.into_iter().next())
    }

    fn package(
        &self,
        info: &SnapInfo,
        local: Option<&SnapInfo>,
        mount_dirs: &[PathBuf],
    ) -> Package {
        let p = to_package(info, local, mount_dirs);
        if let Some(c) = p
            .facts
            .iter()
            .find(|(k, _)| k == "Confinement")
            .map(|(_, v)| v.clone())
        {
            self.confinement
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(p.id.clone(), c);
        }
        p
    }

    /// Whether `snap install` needs `--classic`, from what this source has
    /// already seen, else from one store lookup. Unknown is treated as
    /// strict: `snap install` then says exactly why it refused, which beats
    /// installing without a sandbox on a guess.
    fn is_classic(&self, name: &str) -> bool {
        if let Some(c) = self
            .confinement
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
        {
            return c == "classic";
        }
        if self.transport.presence() != Presence::Socket {
            return false;
        }
        match self.find_one(name) {
            Ok(Some(info)) => {
                self.package(&info, None, &[]);
                info.confinement == "classic"
            }
            Ok(None) => false,
            Err(e) => {
                log::warn!("could not read {name}'s confinement from the Snap Store: {e}");
                false
            }
        }
    }

    fn own<'a>(&self, package: &'a PackageRef) -> Result<&'a str> {
        if package.source == SourceKind::Snap {
            Ok(&package.id)
        } else {
            Err(snap_error(format!(
                "{} is a {} package, not a snap.",
                package.id,
                package.source.label()
            )))
        }
    }
}

fn decode<T: serde::de::DeserializeOwned>(env: Envelope, what: &str) -> Result<T> {
    if env.kind == "error" || env.status_code >= 400 {
        let err: ApiError = serde_json::from_value(env.result).unwrap_or_default();
        return Err(snap_error(api_sentence(&err, env.status_code)));
    }
    serde_json::from_value(env.result).map_err(|e| {
        snap_error(format!(
            "snapd sent something that was not the expected JSON for {what}: {e}."
        ))
    })
}

fn api_sentence(err: &ApiError, status: u16) -> String {
    match err.kind.as_str() {
        "network-timeout" => "The Snap Store did not answer in time. Try again.".to_string(),
        "dns-failure" => {
            "The Snap Store could not be found on the network. Check the connection.".to_string()
        }
        "bad-query" => format!(
            "The Snap Store did not accept that search: {}.",
            err.message.trim_end_matches('.')
        ),
        _ if err.message.is_empty() => format!("snapd answered {status} with no explanation."),
        _ => format!("snapd answered: {}.", err.message.trim_end_matches('.')),
    }
}

fn step(title: String, args: &[&str], weight: u32) -> Step {
    Step {
        source: SourceKind::Snap,
        title,
        command: Command {
            program: "snap".to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            env: Vec::new(),
            cwd: None,
        },
        needs_root: true,
        weight,
    }
}

impl Source for Snap {
    fn kind(&self) -> SourceKind {
        SourceKind::Snap
    }

    fn status(&self) -> SourceStatus {
        let unavailable = |reason: String| SourceStatus {
            kind: SourceKind::Snap,
            available: false,
            reason: Some(reason),
            detail: None,
        };
        match self.transport.presence() {
            Presence::Absent => {
                let mut reason = "snapd is not installed. Install the snapd package to search the Snap Store.".to_string();
                if self.arch_like {
                    reason.push_str(" On Arch it is the snapd package in the AUR.");
                }
                unavailable(reason)
            }
            Presence::ProgramOnly => {
                unavailable("snapd is installed but not running. Start it with systemctl enable --now snapd.socket.".to_string())
            }
            Presence::Socket => match self.system_info() {
                Ok(info) => SourceStatus {
                    kind: SourceKind::Snap,
                    available: true,
                    reason: None,
                    detail: text(&info.version).map(|v| format!("snapd {v}")),
                },
                Err(e) => unavailable(e.message),
            },
        }
    }

    fn search(&self, query: &Query) -> Result<Vec<Package>> {
        let term = query.text.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }
        let found: Vec<SnapInfo> = self.call(&format!("/v2/find?q={}", encode(term)))?;
        // A result says whether it is installed without a second call; if
        // the installed list cannot be read the search still answers.
        let local = self.local_map().unwrap_or_else(|e| {
            log::warn!("could not read the installed snaps: {e}");
            HashMap::new()
        });
        let mount_dirs = self.mount_dirs();
        Ok(found
            .iter()
            .take(query.limit)
            .map(|s| self.package(s, local.get(&s.name), &mount_dirs))
            .collect())
    }

    fn installed(&self) -> Result<Vec<Package>> {
        let snaps: Vec<SnapInfo> = self.call("/v2/snaps")?;
        if !self.mount_dir_known.load(Ordering::Relaxed) {
            // The icon lookup wants snapd's mount directory first; it is
            // cheap to ask once and wrong to guess on Arch or Fedora. A
            // failure here is not one: the guesses stay and the icon is
            // simply not found.
            let _ = self.system_info();
        }
        let mount_dirs = self.mount_dirs();
        Ok(snaps
            .iter()
            .map(|s| self.package(s, Some(s), &mount_dirs))
            .collect())
    }

    fn updates(&self) -> Result<Vec<Update>> {
        let fresh: Vec<SnapInfo> = self.call("/v2/find?select=refresh")?;
        if fresh.is_empty() {
            return Ok(Vec::new());
        }
        let local = self.local_map().unwrap_or_else(|e| {
            log::warn!("could not read the installed snaps: {e}");
            HashMap::new()
        });
        let mount_dirs = self.mount_dirs();
        Ok(fresh
            .iter()
            .map(|s| {
                let installed = local.get(&s.name);
                let p = self.package(s, installed, &mount_dirs);
                Update {
                    package: p.reference(),
                    name: p.name,
                    kind: p.kind,
                    summary: p.summary,
                    icon: p.icon,
                    from: installed.and_then(|l| text(&l.version)),
                    to: s.version.clone(),
                    download_size: size(s.download_size),
                    published: None,
                    is_self: false,
                }
            })
            .collect())
    }

    fn details(&self, id: &str) -> Result<Package> {
        let local = self.local_one(id)?;
        let mount_dirs = self.mount_dirs();
        match (local, self.find_one(id)) {
            (local, Ok(Some(remote))) => Ok(self.package(&remote, local.as_ref(), &mount_dirs)),
            (Some(local), Ok(None)) => Ok(self.package(&local, Some(&local), &mount_dirs)),
            (Some(local), Err(e)) => {
                // Installed is enough for a detail page; the store's extra
                // pictures can wait for a better connection.
                log::warn!("the Snap Store could not answer for {id}: {e}");
                Ok(self.package(&local, Some(&local), &mount_dirs))
            }
            (None, Ok(None)) => Err(snap_error(format!(
                "{id} is not in the Snap Store and is not installed."
            ))),
            (None, Err(e)) => Err(e),
        }
    }

    fn plan(&self, op: &Op) -> Result<Vec<Step>> {
        match op {
            Op::Install { package } => {
                let name = self.own(package)?;
                let mut args = vec!["install", name];
                if self.is_classic(name) {
                    args.push("--classic");
                }
                Ok(vec![step(
                    format!("Installing {name} from the Snap Store"),
                    &args,
                    6,
                )])
            }
            Op::Remove { package } => {
                let name = self.own(package)?;
                Ok(vec![step(format!("Removing {name}"), &["remove", name], 2)])
            }
            Op::Update { package } => {
                let name = self.own(package)?;
                Ok(vec![step(
                    format!("Updating {name} from the Snap Store"),
                    &["refresh", name],
                    6,
                )])
            }
            Op::UpdateAll { source } if *source == SourceKind::Snap => Ok(vec![step(
                "Updating every snap".to_string(),
                &["refresh"],
                8,
            )]),
            Op::UpdateAll { source } => Err(snap_error(format!(
                "Updating every {} package is not a Snap operation.",
                source.label()
            ))),
            // snapd keeps its own catalogue fresh; there is nothing to refresh.
            Op::Refresh { .. } => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn a_content_length_body_reads_back() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"result\":[]}";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"{\"result\":[]}");
    }

    #[test]
    fn a_chunked_body_reads_back_with_extensions_and_trailers() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4;ext=1\r\n{\"re\r\n9\r\nsult\":[]}\r\n0\r\nX-Trailer: yes\r\n\r\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"{\"result\":[]}");
    }

    #[test]
    fn a_body_without_a_length_runs_to_the_end() {
        let raw = b"HTTP/1.0 404 Not Found\r\nConnection: close\r\n\r\nnothing here";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 404);
        assert_eq!(r.body, b"nothing here");
    }

    #[test]
    fn header_names_are_case_insensitive() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok";
        assert_eq!(parse_response(raw).unwrap().body, b"ok");
        let raw = b"HTTP/1.1 200 OK\r\nTRANSFER-ENCODING: Chunked\r\n\r\n2\r\nok\r\n0\r\n\r\n";
        assert_eq!(parse_response(raw).unwrap().body, b"ok");
    }

    #[test]
    fn a_short_body_is_reported_not_returned() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nshort";
        let e = parse_response(raw).unwrap_err();
        assert!(
            e.message.contains("before its answer was complete"),
            "{}",
            e.message
        );
        assert_eq!(e.source_kind, Some(SourceKind::Snap));
    }

    #[test]
    fn something_that_is_not_http_is_reported() {
        let e = parse_response(b"<html>oops</html>\r\n\r\n").unwrap_err();
        assert!(e.message.contains("not HTTP"), "{}", e.message);
        let e = parse_response(b"").unwrap_err();
        assert!(e.message.contains("closed the connection"), "{}", e.message);
        let e = parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n")
            .unwrap_err();
        assert!(e.message.contains("chunk size"), "{}", e.message);
    }

    #[test]
    fn the_request_is_a_closed_http_1_1_get_for_localhost() {
        let r = request("/v2/find?q=firefox");
        assert!(r.starts_with("GET /v2/find?q=firefox HTTP/1.1\r\n"));
        assert!(r.contains("\r\nHost: localhost\r\n"));
        assert!(r.contains("\r\nConnection: close\r\n"));
        assert!(r.ends_with("\r\n\r\n"));
    }

    #[test]
    fn search_text_is_percent_encoded() {
        assert_eq!(encode("firefox"), "firefox");
        assert_eq!(encode("visual studio code"), "visual%20studio%20code");
        assert_eq!(encode("c++ & ünïcode"), "c%2B%2B%20%26%20%C3%BCn%C3%AFcode");
        assert_eq!(encode("a-b.c_d~e"), "a-b.c_d~e");
    }

    #[test]
    fn install_dates_become_unix_seconds() {
        assert_eq!(rfc3339_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_to_unix("2024-05-13T09:12:33Z"), Some(1_715_591_553));
        assert_eq!(
            rfc3339_to_unix("2024-05-13T09:12:33.123456789Z"),
            Some(1_715_591_553)
        );
        assert_eq!(
            rfc3339_to_unix("2026-08-30T14:22:07.517389211+02:00"),
            Some(1_788_092_527)
        );
        assert_eq!(
            rfc3339_to_unix("2026-08-30T14:22:07-05:30"),
            Some(1_788_119_527)
        );
        assert_eq!(rfc3339_to_unix("2000-02-29T23:59:59Z"), Some(951_868_799));
        assert_eq!(rfc3339_to_unix(""), None);
        assert_eq!(rfc3339_to_unix("yesterday"), None);
        assert_eq!(rfc3339_to_unix("2024-13-01T00:00:00Z"), None);
        assert_eq!(rfc3339_to_unix("2024-05-13T09:12:33+0200"), None);
    }

    fn info(kind: &str) -> SnapInfo {
        SnapInfo {
            name: "thing".into(),
            kind: kind.into(),
            ..SnapInfo::default()
        }
    }

    #[test]
    fn kinds_follow_type_and_a_desktop_hint() {
        assert_eq!(kind_of(&info("app"), None), PackageKind::Package);
        let mut with_icon = info("app");
        with_icon.icon = "https://x/icon.png".into();
        assert_eq!(kind_of(&with_icon, None), PackageKind::App);
        let mut with_media = info("app");
        with_media.media.push(Media {
            kind: "icon".into(),
            url: "https://x/i.png".into(),
            ..Media::default()
        });
        assert_eq!(kind_of(&with_media, None), PackageKind::App);
        let mut with_desktop = info("app");
        with_desktop.apps.push(AppInfo {
            name: "thing".into(),
            desktop_file: "/var/lib/snapd/desktop/applications/thing_thing.desktop".into(),
            ..AppInfo::default()
        });
        assert_eq!(kind_of(&info("app"), Some(&with_desktop)), PackageKind::App);
        let mut cli = info("app");
        cli.apps.push(AppInfo {
            name: "thing".into(),
            ..AppInfo::default()
        });
        assert_eq!(kind_of(&cli, Some(&cli)), PackageKind::Package);
        assert_eq!(kind_of(&info("base"), None), PackageKind::Runtime);
        assert_eq!(kind_of(&info("os"), None), PackageKind::Runtime);
        assert_eq!(kind_of(&info("snapd"), None), PackageKind::Runtime);
        assert_eq!(kind_of(&info("kernel"), None), PackageKind::Package);
    }

    #[test]
    fn channels_are_written_in_full() {
        assert_eq!(full_channel("stable").as_deref(), Some("latest/stable"));
        assert_eq!(full_channel("latest/edge").as_deref(), Some("latest/edge"));
        assert_eq!(full_channel("esr/stable").as_deref(), Some("esr/stable"));
        assert_eq!(full_channel(""), None);
    }

    #[test]
    fn revisions_read_as_strings_or_numbers() {
        let s: SnapInfo = serde_json::from_str(r#"{"name":"a","revision":"x1"}"#).unwrap();
        assert_eq!(s.revision, "x1");
        let s: SnapInfo = serde_json::from_str(r#"{"name":"a","revision":42}"#).unwrap();
        assert_eq!(s.revision, "42");
        let s: SnapInfo = serde_json::from_str(r#"{"name":"a"}"#).unwrap();
        assert_eq!(s.revision, "");
    }

    #[test]
    fn a_real_socket_round_trip_sends_the_request_and_reads_a_chunked_answer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapd.socket");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_text = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                request_text.push_str(&line);
            }
            let body =
                r#"{"type":"sync","status-code":200,"status":"OK","result":{"version":"2.63"}}"#;
            let mut answer = String::from(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
            );
            for chunk in body.as_bytes().chunks(16) {
                answer.push_str(&format!("{:x}\r\n", chunk.len()));
                answer.push_str(std::str::from_utf8(chunk).unwrap());
                answer.push_str("\r\n");
            }
            answer.push_str("0\r\n\r\n");
            stream.write_all(answer.as_bytes()).unwrap();
            request_text
        });
        let transport = SocketTransport::at(&path);
        assert_eq!(transport.presence(), Presence::Socket);
        let r = transport.get("/v2/system-info").unwrap();
        assert_eq!(r.status, 200);
        let env: Envelope = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(env.status_code, 200);
        assert_eq!(env.result["version"], "2.63");
        let sent = server.join().unwrap();
        assert!(
            sent.starts_with("GET /v2/system-info HTTP/1.1\r\n"),
            "{sent}"
        );
        assert!(sent.contains("Host: localhost\r\n"), "{sent}");
    }

    #[test]
    fn a_socket_nobody_listens_on_says_snapd_is_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapd.socket");
        drop(UnixListener::bind(&path).unwrap());
        let transport = SocketTransport::at(&path);
        assert_eq!(transport.presence(), Presence::Socket);
        let e = transport.get("/v2/system-info").unwrap_err();
        assert!(
            e.message.starts_with("snapd is not running."),
            "{}",
            e.message
        );
        assert!(
            e.message.contains("systemctl enable --now snapd.socket"),
            "{}",
            e.message
        );
        let system = crate::system::from_os_release("ID=cachyos\nID_LIKE=arch\n");
        let source = Snap::with_transport(&system, Box::new(transport));
        let status = source.status();
        assert!(!status.available);
        assert!(status.reason.unwrap().starts_with("snapd is not running."));
    }

    #[test]
    fn a_missing_socket_says_not_installed_and_names_the_aur_only_on_arch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapd.socket");
        // The machine's PATH decides between "not installed" and "not
        // started" for the real transport; both sentences are checked
        // through the canned transports in tests/snap.rs. Here only the
        // socket half is real.
        let transport = SocketTransport::at(&path);
        if crate::system::which("snap").is_some() {
            assert_eq!(transport.presence(), Presence::ProgramOnly);
        } else {
            assert_eq!(transport.presence(), Presence::Absent);
        }
        let e = transport.get("/v2/snaps").unwrap_err();
        assert!(e.message.contains("is missing"), "{}", e.message);
    }
}
