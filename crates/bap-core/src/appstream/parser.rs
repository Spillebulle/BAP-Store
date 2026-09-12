//! The streaming reader: one pass over a catalogue file, never a tree.
//!
//! `quick-xml` hands out events; a small stack of element tags says where
//! in the document each one is, so `<description>` under `<release>` is
//! not the component's description and `<id>` under `<provides>` is not
//! its id. Subtrees the store does not draw from (`<provides>`,
//! `<content_rating>`, `<languages>`, every translated element) are skipped
//! with `read_to_end_into`, which is the difference between parsing 36 MB
//! and parsing the 6 MB of it that matter.
//!
//! The description is re-serialised as it is read. The alternative, keeping
//! the raw inner XML, was rejected because the catalogue indents it with
//! newlines and runs of spaces inside `<p>` that the page would otherwise
//! have to clean up, and because re-serialising is what limits the markup
//! to the tags the page renders.

use super::Component;
use super::icons::{self, IconDir};
use crate::model::{Picture, Screenshot};
use flate2::read::MultiGzDecoder;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Parse one file's bytes, gzip or plain. The gzip case decompresses as it
/// reads, so the 36 MB plain form of the Arch catalogue is never in memory
/// at once.
pub(super) fn parse(
    bytes: &[u8],
    icons_dir: Option<&Path>,
    origin: Option<&str>,
) -> crate::Result<Vec<Component>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        // MultiGzDecoder rather than GzDecoder: a catalogue written in
        // several gzip members (some generators append) is still one file.
        let decoder = BufReader::with_capacity(1 << 16, MultiGzDecoder::new(bytes));
        parse_reader(decoder, icons_dir, origin)
    } else {
        parse_reader(bytes, icons_dir, origin)
    }
}

fn parse_reader<R: BufRead>(
    input: R,
    icons_dir: Option<&Path>,
    origin: Option<&str>,
) -> crate::Result<Vec<Component>> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::with_capacity(1 << 16);
    let mut skip = Vec::with_capacity(1 << 16);
    let mut parser = Parser::new(icons_dir, origin);
    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| xml_error(reader.error_position(), &e))?;
        match event {
            Event::Start(e) => {
                if parser.start(&e) == Descend::Skip {
                    reader
                        .read_to_end_into(e.name(), &mut skip)
                        .map_err(|e| xml_error(reader.error_position(), &e))?;
                }
            }
            Event::Empty(e) => parser.empty(&e),
            Event::End(e) => parser.end(e.local_name().into_inner()),
            Event::Text(t) => parser.text(&t),
            Event::CData(t) => parser.text(&t),
            Event::GeneralRef(r) => match r.resolve_char_ref() {
                Ok(Some(ch)) => parser.text(ch.encode_utf8(&mut [0; 4])),
                _ => match resolve_predefined_entity(&r) {
                    Some(s) => parser.text(s),
                    // An entity the catalogue did not define; keeping it
                    // as written is the honest rendering.
                    None => parser.text(&format!("&{};", &*r)),
                },
            },
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
        buf.clear();
    }
    parser.finish()
}

fn xml_error(position: u64, e: &quick_xml::Error) -> crate::Error {
    let what = match e {
        quick_xml::Error::Io(io) => format!("The AppStream catalogue could not be read: {io}."),
        other => {
            format!("The AppStream catalogue is not well-formed XML at byte {position}: {other}.")
        }
    };
    crate::Error::new(format!("{what} Refresh the catalogue and try again."))
}

/// The elements the parser descends into; anything else is skipped whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Components,
    Component,
    Id,
    Pkgname,
    Bundle,
    Name,
    Summary,
    Description,
    Developer,
    DeveloperName,
    ProjectLicense,
    Url,
    Categories,
    Category,
    Keywords,
    Keyword,
    Icon,
    Screenshots,
    Screenshot,
    Image,
    Caption,
    Releases,
    Release,
    Other,
}

impl Tag {
    fn of(name: &str) -> Tag {
        match name {
            "components" => Tag::Components,
            "component" => Tag::Component,
            "id" => Tag::Id,
            "pkgname" => Tag::Pkgname,
            "bundle" => Tag::Bundle,
            "name" => Tag::Name,
            "summary" => Tag::Summary,
            "description" => Tag::Description,
            "developer" => Tag::Developer,
            "developer_name" => Tag::DeveloperName,
            "project_license" => Tag::ProjectLicense,
            "url" => Tag::Url,
            "categories" => Tag::Categories,
            "category" => Tag::Category,
            "keywords" => Tag::Keywords,
            "keyword" => Tag::Keyword,
            "icon" => Tag::Icon,
            "screenshots" => Tag::Screenshots,
            "screenshot" => Tag::Screenshot,
            "image" => Tag::Image,
            "caption" => Tag::Caption,
            "releases" => Tag::Releases,
            "release" => Tag::Release,
            _ => Tag::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Descend {
    Into,
    Skip,
}

#[derive(Clone, Copy, Debug)]
enum IconKind {
    Cached,
    Remote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageKind {
    Source,
    Thumbnail,
}

/// What the file says about itself, applied to every component in it.
struct FileFacts<'a> {
    icons_dir: Option<&'a Path>,
    /// The caller's origin wins over the file's, because the caller knows
    /// which Flatpak remote a file belongs to and the file may not.
    origin_override: Option<&'a str>,
    origin: String,
    media_base: Option<String>,
    icon_dir: Option<IconDir>,
}

impl<'a> FileFacts<'a> {
    fn new(icons_dir: Option<&'a Path>, origin: Option<&'a str>) -> FileFacts<'a> {
        let mut facts = FileFacts {
            icons_dir,
            origin_override: origin,
            origin: String::new(),
            media_base: None,
            icon_dir: None,
        };
        facts.settle(None, None);
        facts
    }

    fn root(&mut self, e: &BytesStart) {
        self.settle(attr(e, "origin"), attr(e, "media_baseurl"));
    }

    fn settle(&mut self, file_origin: Option<String>, media_base: Option<String>) {
        self.origin = self
            .origin_override
            .map(str::to_string)
            .or(file_origin)
            .unwrap_or_default();
        self.media_base = media_base.filter(|b| !b.trim().is_empty());
        self.icon_dir = IconDir::new(self.icons_dir, &self.origin);
    }
}

struct ImageDraft {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Default)]
struct ShotDraft {
    default: bool,
    caption: Option<String>,
    source: Option<ImageDraft>,
    thumbnails: Vec<ImageDraft>,
}

impl ShotDraft {
    /// The page shows `image` in the viewer and `thumbnail` in rails; a
    /// screenshot with thumbnails and no source uses its largest thumbnail
    /// as the image rather than being dropped.
    fn finish(self) -> Option<Screenshot> {
        let largest = self
            .thumbnails
            .into_iter()
            .max_by_key(|t| t.width.unwrap_or(0));
        let (image, thumbnail) = match (self.source, largest) {
            (Some(source), thumbnail) => (source, thumbnail),
            (None, Some(thumbnail)) => (thumbnail, None),
            (None, None) => return None,
        };
        Some(Screenshot {
            image: Picture::Url(image.url),
            thumbnail: thumbnail.map(|t| Picture::Url(t.url)),
            caption: self.caption,
            width: image.width,
            height: image.height,
        })
    }
}

#[derive(Default)]
struct Draft {
    component: Component,
    cached_icons: Vec<(u32, String)>,
    /// The widest remote icon, already made absolute.
    remote_icon: Option<(u32, String)>,
    /// `<developer><name>` is the current form; `<developer_name>` the one
    /// it replaced. Catalogues carry both for a while; the current one wins.
    developer: Option<String>,
    developer_name: Option<String>,
    shots: Vec<(bool, Screenshot)>,
    shot: Option<ShotDraft>,
    release: Option<(String, Option<i64>)>,
}

impl Draft {
    fn note_release(&mut self, version: String, timestamp: Option<i64>) {
        let newer = match &self.release {
            None => true,
            Some((_, Some(best))) => timestamp.is_some_and(|t| t > *best),
            // The best so far is undated; a dated one is better evidence of
            // being newest than the file order.
            Some((_, None)) => timestamp.is_some(),
        };
        if newer {
            self.release = Some((version, timestamp));
        }
    }

    fn finish(mut self, facts: &FileFacts) -> Option<Component> {
        if self.component.id.is_empty() {
            log::debug!(
                "appstream: a component in {} has no id; skipped",
                facts.origin
            );
            return None;
        }
        if self.component.name.is_empty() {
            self.component.name = self.component.id.clone();
        }
        self.component.developer = self.developer.or(self.developer_name);
        self.component.icon = icons::pick(
            &self.cached_icons,
            self.remote_icon.as_ref().map(|(_, u)| u.as_str()),
            facts.icon_dir.as_ref(),
        );
        // The catalogue marks one screenshot as the default; the page's
        // backdrop is the first, so the default goes first.
        if let Some(i) = self.shots.iter().position(|(default, _)| *default) {
            let default = self.shots.remove(i);
            self.shots.insert(0, default);
        }
        self.component.screenshots = self.shots.into_iter().map(|(_, s)| s).collect();
        self.component.latest_release = self.release;
        Some(self.component)
    }
}

struct Parser<'a> {
    facts: FileFacts<'a>,
    stack: Vec<Tag>,
    /// Text of the leaf element being read.
    text: String,
    draft: Option<Draft>,
    /// Active while inside the component's `<description>`; every event
    /// until its end goes here instead of `text`.
    description: Option<DescriptionWriter>,
    /// Stack depth just inside the `<description>`, to tell its own end tag
    /// from those of the paragraphs in it.
    description_depth: usize,
    icon: Option<(IconKind, u32)>,
    image: Option<(ImageKind, Option<u32>, Option<u32>)>,
    out: Vec<Component>,
}

impl<'a> Parser<'a> {
    fn new(icons_dir: Option<&'a Path>, origin: Option<&'a str>) -> Parser<'a> {
        Parser {
            facts: FileFacts::new(icons_dir, origin),
            stack: Vec::with_capacity(8),
            text: String::new(),
            draft: None,
            description: None,
            description_depth: 0,
            icon: None,
            image: None,
            out: Vec::new(),
        }
    }

    fn start(&mut self, e: &BytesStart) -> Descend {
        if is_translated(e) {
            return Descend::Skip;
        }
        let name = e.local_name();
        let name = name.into_inner();
        if let Some(description) = &mut self.description {
            description.open(name);
            self.stack.push(Tag::Other);
            return Descend::Into;
        }
        let tag = Tag::of(name);
        let parent = self.stack.last().copied();
        let descend = match (parent, tag) {
            (None, Tag::Components) => {
                self.facts.root(e);
                true
            }
            (None | Some(Tag::Components), Tag::Component) => {
                self.begin_component(e);
                true
            }
            (Some(Tag::Component), tag) => self.start_in_component(e, tag),
            (Some(Tag::Developer), Tag::Name)
            | (Some(Tag::Categories), Tag::Category)
            | (Some(Tag::Keywords), Tag::Keyword)
            | (Some(Tag::Screenshot), Tag::Caption) => {
                self.text.clear();
                true
            }
            (Some(Tag::Screenshots), Tag::Screenshot) => {
                if let Some(draft) = &mut self.draft {
                    draft.shot = Some(ShotDraft {
                        default: attr(e, "type").as_deref() == Some("default"),
                        ..Default::default()
                    });
                }
                true
            }
            (Some(Tag::Screenshot), Tag::Image) => {
                let kind = match attr(e, "type").as_deref() {
                    None | Some("source") => ImageKind::Source,
                    Some("thumbnail") => ImageKind::Thumbnail,
                    Some(_) => return Descend::Skip,
                };
                self.image = Some((kind, number(e, "width"), number(e, "height")));
                self.text.clear();
                true
            }
            (Some(Tag::Releases), Tag::Release) => {
                self.note_release(e);
                // A release carries its own description and issue list;
                // nothing below it is drawn.
                false
            }
            _ => false,
        };
        if descend {
            self.stack.push(tag);
            Descend::Into
        } else {
            Descend::Skip
        }
    }

    fn start_in_component(&mut self, e: &BytesStart, tag: Tag) -> bool {
        match tag {
            Tag::Id
            | Tag::Pkgname
            | Tag::Name
            | Tag::Summary
            | Tag::DeveloperName
            | Tag::ProjectLicense => {
                self.text.clear();
                true
            }
            Tag::Bundle => {
                self.text.clear();
                attr(e, "type").as_deref() == Some("flatpak")
            }
            Tag::Url => {
                self.text.clear();
                attr(e, "type").as_deref() == Some("homepage")
            }
            Tag::Description => {
                self.description = Some(DescriptionWriter::new());
                self.description_depth = self.stack.len() + 1;
                true
            }
            Tag::Developer | Tag::Categories | Tag::Keywords | Tag::Screenshots | Tag::Releases => {
                true
            }
            Tag::Icon => {
                let kind = match attr(e, "type").as_deref() {
                    Some("cached") => IconKind::Cached,
                    Some("remote") => IconKind::Remote,
                    // Stock icons name a theme icon the page cannot look up
                    // from a webview; local ones are paths of another
                    // machine's catalogue generator.
                    _ => return false,
                };
                // The specification's default for a cached icon without a
                // width is 64 px.
                self.icon = Some((kind, number(e, "width").unwrap_or(64)));
                self.text.clear();
                true
            }
            _ => false,
        }
    }

    fn empty(&mut self, e: &BytesStart) {
        if is_translated(e) || self.description.is_some() {
            return;
        }
        let name = e.local_name();
        if self.stack.last() == Some(&Tag::Releases) && Tag::of(name.into_inner()) == Tag::Release {
            self.note_release(e);
        }
    }

    fn end(&mut self, name: &str) {
        let Some(tag) = self.stack.pop() else {
            return;
        };
        if self.description.is_some() {
            if self.stack.len() + 1 == self.description_depth {
                let description = self.description.take().and_then(DescriptionWriter::finish);
                if let Some(draft) = &mut self.draft {
                    draft.component.description = description;
                }
            } else if let Some(description) = &mut self.description {
                description.close(name);
            }
            return;
        }
        if tag == Tag::Component {
            if let Some(component) = self.draft.take().and_then(|d| d.finish(&self.facts)) {
                self.out.push(component);
            }
            return;
        }
        let value = self.text.trim();
        let Some(draft) = &mut self.draft else {
            return;
        };
        match tag {
            Tag::Screenshot => {
                if let Some(shot) = draft.shot.take() {
                    let default = shot.default;
                    if let Some(screenshot) = shot.finish() {
                        draft.shots.push((default, screenshot));
                    }
                }
            }
            Tag::Icon => {
                if let Some((kind, width)) = self.icon.take()
                    && !value.is_empty()
                {
                    match kind {
                        IconKind::Cached => draft.cached_icons.push((width, value.to_string())),
                        IconKind::Remote => {
                            if let Some(url) = absolute(value, self.facts.media_base.as_deref())
                                && draft.remote_icon.as_ref().is_none_or(|(w, _)| width > *w)
                            {
                                draft.remote_icon = Some((width, url));
                            }
                        }
                    }
                }
            }
            Tag::Image => {
                if let Some((kind, width, height)) = self.image.take()
                    && let Some(shot) = &mut draft.shot
                    && let Some(url) = absolute(value, self.facts.media_base.as_deref())
                {
                    let image = ImageDraft { url, width, height };
                    match kind {
                        ImageKind::Source => shot.source = Some(image),
                        ImageKind::Thumbnail => shot.thumbnails.push(image),
                    }
                }
            }
            _ if value.is_empty() => {}
            Tag::Id => {
                draft.component.id = value.strip_suffix(".desktop").unwrap_or(value).to_string()
            }
            Tag::Pkgname => draft.component.pkgname = Some(value.to_string()),
            Tag::Bundle => draft.component.bundle = Some(value.to_string()),
            Tag::Name => {
                if self.stack.last() == Some(&Tag::Developer) {
                    draft.developer = Some(value.to_string());
                } else {
                    draft.component.name = value.to_string();
                }
            }
            Tag::Summary => draft.component.summary = Some(value.to_string()),
            Tag::DeveloperName => draft.developer_name = Some(value.to_string()),
            Tag::ProjectLicense => draft.component.licence = Some(value.to_string()),
            Tag::Url => draft.component.homepage = Some(value.to_string()),
            Tag::Category => draft.component.categories.push(value.to_string()),
            Tag::Keyword => draft.component.keywords.push(value.to_string()),
            Tag::Caption => {
                if let Some(shot) = &mut draft.shot {
                    shot.caption = Some(value.to_string());
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        match &mut self.description {
            Some(description) => description.text(s),
            None => self.text.push_str(s),
        }
    }

    fn begin_component(&mut self, e: &BytesStart) {
        let component_type = attr(e, "type").unwrap_or_else(|| "generic".to_string());
        let mut draft = Draft::default();
        draft.component.origin = self.facts.origin.clone();
        draft.component.is_app = is_app(&component_type);
        draft.component.component_type = component_type;
        self.draft = Some(draft);
    }

    fn note_release(&mut self, e: &BytesStart) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        let Some(version) = attr(e, "version").filter(|v| !v.trim().is_empty()) else {
            return;
        };
        let timestamp = attr(e, "timestamp")
            .and_then(|t| t.trim().parse::<i64>().ok())
            .or_else(|| attr(e, "date").and_then(|d| date_to_unix(&d)));
        draft.note_release(version.trim().to_string(), timestamp);
    }

    fn finish(self) -> crate::Result<Vec<Component>> {
        if !self.stack.is_empty() {
            return Err(crate::Error::new(
                "The AppStream catalogue ends before its root element is closed, so it is probably truncated. Refresh the catalogue and try again.",
            ));
        }
        Ok(self.out)
    }
}

/// `desktop` is what Flathub's older catalogues call `desktop-application`.
fn is_app(component_type: &str) -> bool {
    matches!(
        component_type,
        "desktop-application" | "console-application" | "web-application" | "desktop"
    )
}

fn is_translated(e: &BytesStart) -> bool {
    attr(e, "xml:lang").is_some_and(|lang| !lang.trim().is_empty())
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    let a = e.try_get_attribute(key).ok().flatten()?;
    a.normalized_value(XmlVersion::Implicit1_0)
        .ok()
        .map(|v| v.into_owned())
}

fn number(e: &BytesStart, key: &str) -> Option<u32> {
    attr(e, key).and_then(|v| v.trim().parse().ok())
}

/// A URL the page can fetch: as written when it has a scheme, joined to
/// the file's `media_baseurl` when relative, nothing when relative and
/// there is no base (the Arch catalogue's remote icons are like that).
fn absolute(url: &str, base: Option<&str>) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    if url.contains("://") {
        return Some(url.to_string());
    }
    let base = base?.trim().trim_end_matches('/');
    Some(format!("{base}/{}", url.trim_start_matches('/')))
}

/// `YYYY-MM-DD`, optionally followed by `THH:MM:SS` and a zone the value
/// is taken to be UTC in, to Unix seconds. Days from the civil calendar by
/// the usual era arithmetic; a date library would be a dependency for one
/// attribute that most catalogues write as `timestamp` anyway.
fn date_to_unix(s: &str) -> Option<i64> {
    let s = s.trim();
    let date = s.get(..10)?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut seconds = days_from_civil(year, month, day) * 86_400;
    if let Some(time) = s.get(10..).and_then(|rest| rest.strip_prefix('T')) {
        let mut clock = time
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty());
        let hour: i64 = clock.next()?.parse().ok()?;
        let minute: i64 = clock.next().and_then(|m| m.parse().ok()).unwrap_or(0);
        let second: i64 = clock.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        seconds += hour * 3600 + minute * 60 + second;
    }
    Some(seconds)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_from_march = i64::from((month + 9) % 12);
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Re-serialises a description as it streams past: the six tags the page
/// renders are kept, every other tag is dropped with its text kept, and
/// whitespace runs collapse to one space with none at the edges of a block.
struct DescriptionWriter {
    out: String,
    /// Just after a block tag, where leading whitespace is indentation.
    at_block_edge: bool,
}

impl DescriptionWriter {
    fn new() -> DescriptionWriter {
        DescriptionWriter {
            out: String::new(),
            at_block_edge: true,
        }
    }

    fn open(&mut self, tag: &str) {
        match tag {
            "p" | "ul" | "ol" | "li" => {
                self.out.push('<');
                self.out.push_str(tag);
                self.out.push('>');
                self.at_block_edge = true;
            }
            "em" | "code" => {
                self.out.push('<');
                self.out.push_str(tag);
                self.out.push('>');
            }
            _ => {}
        }
    }

    fn close(&mut self, tag: &str) {
        match tag {
            "p" | "ul" | "ol" | "li" => {
                let trimmed = self.out.trim_end().len();
                self.out.truncate(trimmed);
                self.out.push_str("</");
                self.out.push_str(tag);
                self.out.push('>');
                if tag != "li" {
                    self.out.push('\n');
                }
                self.at_block_edge = true;
            }
            "em" | "code" => {
                self.out.push_str("</");
                self.out.push_str(tag);
                self.out.push('>');
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        let collapsed = collapse_whitespace(s);
        let piece = if self.at_block_edge {
            collapsed.trim_start()
        } else {
            collapsed.as_str()
        };
        if piece.is_empty() {
            return;
        }
        for ch in piece.chars() {
            match ch {
                '&' => self.out.push_str("&amp;"),
                '<' => self.out.push_str("&lt;"),
                '>' => self.out.push_str("&gt;"),
                _ => self.out.push(ch),
            }
        }
        self.at_block_edge = false;
    }

    fn finish(self) -> Option<String> {
        let trimmed = self.out.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_run {
                out.push(' ');
                in_run = true;
            }
        } else {
            out.push(ch);
            in_run = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(xml: &str) -> Component {
        let mut found = parse(xml.as_bytes(), None, None).unwrap();
        assert_eq!(found.len(), 1, "expected one component in {xml}");
        found.remove(0)
    }

    fn wrap(inner: &str) -> String {
        format!(
            "<components origin=\"test\"><component type=\"desktop-application\"><id>a.b</id>{inner}</component></components>"
        )
    }

    #[test]
    fn description_markup_is_kept_and_whitespace_collapsed() {
        let c = one(&wrap(
            "<description>\n    <p>\n      GIMP is   an acronym.\n      It is <em>Free</em> Software &amp; more.\n    </p>\n    <p xml:lang=\"de\">Nicht.</p>\n    <ul>\n      <li>One &lt;thing&gt;</li>\n      <li><code>two</code> </li>\n    </ul>\n    <p>Last</p>\n  </description>",
        ));
        assert_eq!(
            c.description.as_deref(),
            Some(
                "<p>GIMP is an acronym. It is <em>Free</em> Software &amp; more.</p>\n<ul><li>One &lt;thing&gt;</li><li><code>two</code></li></ul>\n<p>Last</p>"
            )
        );
    }

    #[test]
    fn unknown_tags_in_a_description_are_dropped_but_their_text_kept() {
        let c = one(&wrap(
            "<description><p>See <a href=\"x\">the site</a> now.</p><ol><li>a</li></ol></description>",
        ));
        assert_eq!(
            c.description.as_deref(),
            Some("<p>See the site now.</p>\n<ol><li>a</li></ol>")
        );
    }

    #[test]
    fn an_empty_description_is_none() {
        assert_eq!(
            one(&wrap("<description>  </description>")).description,
            None
        );
        assert_eq!(one(&wrap("<description/>")).description, None);
    }

    #[test]
    fn a_release_description_is_not_the_component_description() {
        let c = one(&wrap(
            "<releases><release version=\"2\" timestamp=\"20\"><description><p>notes</p></description></release><release version=\"1\" timestamp=\"10\"/></releases><description><p>real</p></description>",
        ));
        assert_eq!(c.description.as_deref(), Some("<p>real</p>"));
        assert_eq!(c.latest_release, Some(("2".into(), Some(20))));
    }

    #[test]
    fn the_newest_release_wins_by_timestamp_or_date_regardless_of_order() {
        let c = one(&wrap(
            "<releases><release version=\"1\" timestamp=\"10\"/><release version=\"3\" date=\"2024-01-15\"/><release version=\"2\" timestamp=\"20\"/></releases>",
        ));
        assert_eq!(c.latest_release, Some(("3".into(), Some(1_705_276_800))));
        let undated = one(&wrap(
            "<releases><release version=\"9\"/><release version=\"8\"/></releases>",
        ));
        assert_eq!(undated.latest_release, Some(("9".into(), None)));
        let mixed = one(&wrap(
            "<releases><release version=\"9\"/><release version=\"8\" timestamp=\"5\"/></releases>",
        ));
        assert_eq!(mixed.latest_release, Some(("8".into(), Some(5))));
    }

    #[test]
    fn dates_convert_to_unix_seconds() {
        assert_eq!(date_to_unix("1970-01-01"), Some(0));
        assert_eq!(date_to_unix("2000-03-01"), Some(951_868_800));
        assert_eq!(date_to_unix("2024-01-15"), Some(1_705_276_800));
        assert_eq!(
            date_to_unix("2024-01-15T10:30:15Z"),
            Some(1_705_276_800 + 10 * 3600 + 30 * 60 + 15)
        );
        assert_eq!(date_to_unix("1969-12-31"), Some(-86_400));
        assert_eq!(date_to_unix("2024-13-01"), None);
        assert_eq!(date_to_unix("soon"), None);
    }

    #[test]
    fn translated_elements_are_skipped_wherever_they_are() {
        let c = one(&wrap(
            "<name xml:lang=\"de\">Deutsch</name><name>English</name><name xml:lang=\"fr\">Français</name><summary xml:lang=\"de\">S</summary><summary>Sum</summary><developer><name xml:lang=\"de\">Wer</name><name>Who</name></developer><keywords xml:lang=\"de\"><keyword>eins</keyword></keywords><keywords><keyword>one</keyword></keywords><screenshots><screenshot><caption xml:lang=\"de\">Bild</caption><caption>Picture</caption><image type=\"source\">https://e/1.png</image></screenshot></screenshots>",
        ));
        assert_eq!(c.name, "English");
        assert_eq!(c.summary.as_deref(), Some("Sum"));
        assert_eq!(c.developer.as_deref(), Some("Who"));
        assert_eq!(c.keywords, vec!["one"]);
        assert_eq!(c.screenshots[0].caption.as_deref(), Some("Picture"));
    }

    #[test]
    fn the_id_loses_a_trailing_desktop_and_the_name_falls_back_to_it() {
        let c = one(
            "<components origin=\"o\"><component type=\"desktop-application\"><id>steam.desktop</id></component></components>",
        );
        assert_eq!(c.id, "steam");
        assert_eq!(c.name, "steam");
        let c = one(
            "<components origin=\"o\"><component><id>org.example.desktop.Tool</id><name>T</name></component></components>",
        );
        assert_eq!(c.id, "org.example.desktop.Tool");
        assert_eq!(c.component_type, "generic");
        assert!(!c.is_app);
    }

    #[test]
    fn a_component_without_an_id_is_dropped() {
        let found = parse(b"<components origin=\"o\"><component type=\"desktop-application\"><name>Nameless</name></component></components>", None, None).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn the_callers_origin_wins_over_the_files() {
        let xml = "<components origin=\"in-file\"><component type=\"desktop-application\"><id>a</id></component></components>";
        assert_eq!(
            parse(xml.as_bytes(), None, None).unwrap()[0].origin,
            "in-file"
        );
        assert_eq!(
            parse(xml.as_bytes(), None, Some("caller")).unwrap()[0].origin,
            "caller"
        );
        let bare = "<component type=\"desktop-application\"><id>a</id></component>";
        assert_eq!(
            parse(bare.as_bytes(), None, Some("caller")).unwrap()[0].origin,
            "caller"
        );
        assert_eq!(parse(bare.as_bytes(), None, None).unwrap()[0].origin, "");
    }

    #[test]
    fn component_types_decide_is_app() {
        for (t, app) in [
            ("desktop-application", true),
            ("console-application", true),
            ("web-application", true),
            ("desktop", true),
            ("addon", false),
            ("font", false),
            ("runtime", false),
        ] {
            let c = one(&format!(
                "<components><component type=\"{t}\"><id>a</id></component></components>"
            ));
            assert_eq!(c.is_app, app, "{t}");
            assert_eq!(c.component_type, t);
        }
    }

    #[test]
    fn bundles_and_urls_are_filtered_by_type() {
        let c = one(&wrap(
            "<bundle type=\"limba\">no</bundle><bundle type=\"flatpak\">app/a.b/x86_64/stable</bundle><url type=\"bugtracker\">https://bugs</url><url type=\"homepage\">https://home</url>",
        ));
        assert_eq!(c.bundle.as_deref(), Some("app/a.b/x86_64/stable"));
        assert_eq!(c.homepage.as_deref(), Some("https://home"));
    }

    #[test]
    fn developer_prefers_the_current_form_over_developer_name() {
        let both = one(&wrap(
            "<developer id=\"x\"><name>New</name></developer><developer_name>Old</developer_name>",
        ));
        assert_eq!(both.developer.as_deref(), Some("New"));
        let legacy = one(&wrap("<developer_name>Old</developer_name>"));
        assert_eq!(legacy.developer.as_deref(), Some("Old"));
        assert_eq!(one(&wrap("")).developer, None);
    }

    #[test]
    fn provides_id_is_not_the_component_id_and_licence_and_categories_are_read() {
        let c = one(&wrap(
            "<name>N</name><provides><id>other.id</id><binary>n</binary></provides><project_license>GPL-3.0-or-later</project_license><categories><category>Graphics</category><category>2DGraphics</category></categories>",
        ));
        assert_eq!(c.id, "a.b");
        assert_eq!(c.licence.as_deref(), Some("GPL-3.0-or-later"));
        assert_eq!(c.categories, vec!["Graphics", "2DGraphics"]);
    }

    #[test]
    fn screenshots_keep_sizes_captions_thumbnails_and_put_the_default_first() {
        let c = one(&wrap(
            "<screenshots><screenshot><image type=\"source\" width=\"1200\" height=\"800\">https://e/a.png</image></screenshot><screenshot type=\"default\"><caption>Main</caption><image type=\"source\" width=\"1000\" height=\"600\">https://e/b.png</image><image type=\"thumbnail\" width=\"224\" height=\"134\">https://e/b-224.png</image><image type=\"thumbnail\" width=\"624\" height=\"374\">https://e/b-624.png</image></screenshot><screenshot><image type=\"thumbnail\" width=\"100\">https://e/c-thumb.png</image></screenshot><screenshot><caption>Empty</caption></screenshot></screenshots>",
        ));
        assert_eq!(c.screenshots.len(), 3);
        let first = &c.screenshots[0];
        assert_eq!(first.image, Picture::Url("https://e/b.png".into()));
        assert_eq!(
            first.thumbnail,
            Some(Picture::Url("https://e/b-624.png".into()))
        );
        assert_eq!(first.caption.as_deref(), Some("Main"));
        assert_eq!((first.width, first.height), (Some(1000), Some(600)));
        assert_eq!(
            c.screenshots[1].image,
            Picture::Url("https://e/a.png".into())
        );
        assert_eq!(c.screenshots[1].thumbnail, None);
        assert_eq!(
            c.screenshots[2].image,
            Picture::Url("https://e/c-thumb.png".into())
        );
        assert_eq!(c.screenshots[2].width, Some(100));
    }

    #[test]
    fn relative_urls_join_media_baseurl_or_are_dropped() {
        assert_eq!(
            absolute("https://e/x.png", None).as_deref(),
            Some("https://e/x.png")
        );
        assert_eq!(
            absolute("org/x.png", Some("https://media/")).as_deref(),
            Some("https://media/org/x.png")
        );
        assert_eq!(
            absolute("/org/x.png", Some("https://media")).as_deref(),
            Some("https://media/org/x.png")
        );
        assert_eq!(absolute("org/x.png", None), None);
        assert_eq!(absolute("  ", Some("https://media")), None);
        let xml = "<components origin=\"debian-bookworm-main\" media_baseurl=\"https://appstream.debian.org/media/bookworm\"><component type=\"desktop-application\"><id>a</id><icon type=\"remote\" width=\"64\">a/icons/64x64/a.png</icon><screenshots><screenshot><image type=\"source\">a/screenshots/image-1.png</image></screenshot></screenshots></component></components>";
        let c = parse(xml.as_bytes(), None, None).unwrap().remove(0);
        assert_eq!(
            c.icon,
            Some(Picture::Url(
                "https://appstream.debian.org/media/bookworm/a/icons/64x64/a.png".into()
            ))
        );
        assert_eq!(
            c.screenshots[0].image,
            Picture::Url(
                "https://appstream.debian.org/media/bookworm/a/screenshots/image-1.png".into()
            )
        );
        let no_base = "<components origin=\"o\"><component type=\"desktop-application\"><id>a</id><icon type=\"remote\">rel/a.png</icon></component></components>";
        assert_eq!(parse(no_base.as_bytes(), None, None).unwrap()[0].icon, None);
    }

    #[test]
    fn icons_resolve_against_the_largest_file_on_disk_or_fall_back_to_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        for size in ["48x48", "64x64"] {
            std::fs::create_dir_all(icons.join("test").join(size)).unwrap();
            std::fs::write(icons.join("test").join(size).join("a.png"), b"png").unwrap();
        }
        let xml = wrap(
            "<icon type=\"stock\">a</icon><icon type=\"cached\" width=\"48\" height=\"48\">a.png</icon><icon type=\"cached\" width=\"64\" height=\"64\">a.png</icon><icon type=\"cached\" width=\"128\" height=\"128\">a.png</icon><icon type=\"remote\" width=\"32\">https://e/small.png</icon><icon type=\"remote\" width=\"128\">https://e/big.png</icon>",
        );
        let c = parse(xml.as_bytes(), Some(&icons), None).unwrap().remove(0);
        assert_eq!(c.icon, Some(Picture::File(icons.join("test/64x64/a.png"))));
        let missing = parse(
            xml.replace("a.png", "missing.png").as_bytes(),
            Some(&icons),
            None,
        )
        .unwrap()
        .remove(0);
        assert_eq!(missing.icon, Some(Picture::Url("https://e/big.png".into())));
        let none = parse(xml.as_bytes(), None, None).unwrap().remove(0);
        assert_eq!(
            none.icon,
            Some(Picture::Url("https://e/big.png".into())),
            "no icons directory: the remote one"
        );
        let stock_only = one(&wrap(
            "<icon type=\"stock\">a</icon><icon type=\"local\">/usr/share/pixmaps/a.png</icon>",
        ));
        assert_eq!(stock_only.icon, None);
    }

    #[test]
    fn entities_are_resolved_in_text_and_escaped_again_in_descriptions() {
        let c = one(&wrap(
            "<name>Tools &amp; Toys</name><summary>&lt;b&gt; &#169; &#x41;</summary><description><p>A &amp; B &lt; C</p></description>",
        ));
        assert_eq!(c.name, "Tools & Toys");
        assert_eq!(c.summary.as_deref(), Some("<b> © A"));
        assert_eq!(c.description.as_deref(), Some("<p>A &amp; B &lt; C</p>"));
    }

    #[test]
    fn gzip_and_plain_bytes_parse_the_same() {
        use std::io::Write;
        let xml = wrap("<name>Zipped</name>");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(xml.as_bytes()).unwrap();
        let gz = encoder.finish().unwrap();
        assert_eq!(
            parse(&gz, None, None).unwrap(),
            parse(xml.as_bytes(), None, None).unwrap()
        );
        assert_eq!(parse(&gz, None, None).unwrap()[0].name, "Zipped");
    }

    #[test]
    fn broken_input_is_an_error_that_says_so() {
        let truncated = parse(
            b"<components origin=\"o\"><component><id>a</id>",
            None,
            None,
        )
        .unwrap_err();
        assert!(
            truncated.message.contains("truncated"),
            "{}",
            truncated.message
        );
        let mismatched =
            parse(b"<components><component></wrong></components>", None, None).unwrap_err();
        assert!(
            mismatched.message.contains("not well-formed"),
            "{}",
            mismatched.message
        );
        let bad_gzip = parse(&[0x1f, 0x8b, 0x08, 0x00, 0xff, 0xff], None, None).unwrap_err();
        assert!(
            bad_gzip.message.contains("could not be read"),
            "{}",
            bad_gzip.message
        );
        for e in [&truncated, &mismatched, &bad_gzip] {
            assert!(
                e.message.ends_with("Refresh the catalogue and try again."),
                "{}",
                e.message
            );
        }
        assert!(parse(b"", None, None).unwrap().is_empty());
    }

    #[test]
    fn whitespace_collapses_to_single_spaces() {
        assert_eq!(collapse_whitespace("  a \n\t b  "), " a b ");
        assert_eq!(collapse_whitespace("ab"), "ab");
        assert_eq!(collapse_whitespace(""), "");
    }
}
