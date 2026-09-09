//! One HTTP client for every source, with a user agent that names the
//! application, sensible timeouts, and a small disk cache for the JSON
//! answers that change slowly (Flathub's summaries, AUR info).
//!
//! Blocking on purpose: every source is a plain function run on a worker
//! thread, and a blocking client keeps the sources free of an async runtime.

use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const USER_AGENT: &str = concat!("bap-store/", env!("CARGO_PKG_VERSION"), " (+https://github.com/Spillebulle/BAP-Store)");

#[derive(Clone)]
pub struct Client {
    inner: reqwest::blocking::Client,
    cache_dir: PathBuf,
}

static SHARED: OnceLock<Arc<Client>> = OnceLock::new();

impl Client {
    /// The one client the application uses.
    pub fn shared() -> Arc<Client> {
        SHARED
            .get_or_init(|| Arc::new(Client::new(crate::system::Dirs::new().cache.join("http"))))
            .clone()
    }

    pub fn new(cache_dir: PathBuf) -> Client {
        let inner = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("a client with static settings builds");
        Client { inner, cache_dir }
    }

    pub fn raw(&self) -> &reqwest::blocking::Client {
        &self.inner
    }

    /// GET a URL and return the body as text.
    pub fn get_text(&self, url: &str) -> Result<String> {
        let resp = self
            .inner
            .get(url)
            .send()
            .map_err(|e| Error::new(describe(url, &e)))?;
        if !resp.status().is_success() {
            return Err(Error::new(format!("{} answered {}", host(url), resp.status())));
        }
        resp.text().map_err(|e| Error::new(describe(url, &e)))
    }

    /// GET a URL and return the body as bytes.
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self
            .inner
            .get(url)
            .send()
            .map_err(|e| Error::new(describe(url, &e)))?;
        if !resp.status().is_success() {
            return Err(Error::new(format!("{} answered {}", host(url), resp.status())));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| Error::new(describe(url, &e)))
    }

    /// GET JSON, with extra headers where an API wants them.
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str, headers: &[(&str, &str)]) -> Result<T> {
        let mut req = self.inner.get(url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send().map_err(|e| Error::new(describe(url, &e)))?;
        if !resp.status().is_success() {
            return Err(Error::new(format!("{} answered {}", host(url), resp.status())));
        }
        resp.json::<T>()
            .map_err(|e| Error::new(format!("{} sent something that was not the expected JSON: {e}", host(url))))
    }

    /// POST a JSON body and read a JSON answer.
    pub fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(&self, url: &str, body: &B) -> Result<T> {
        let resp = self
            .inner
            .post(url)
            .json(body)
            .send()
            .map_err(|e| Error::new(describe(url, &e)))?;
        if !resp.status().is_success() {
            return Err(Error::new(format!("{} answered {}", host(url), resp.status())));
        }
        resp.json::<T>()
            .map_err(|e| Error::new(format!("{} sent something that was not the expected JSON: {e}", host(url))))
    }

    /// GET text through the disk cache: the cached copy is used when it is
    /// younger than `max_age`, otherwise the network is asked and the answer
    /// stored. A network failure with a stale copy on disk returns the stale
    /// copy, because an old answer beats no answer for a store's metadata.
    pub fn get_text_cached(&self, url: &str, max_age: Duration) -> Result<String> {
        let path = self.cache_path(url);
        if let Ok(meta) = std::fs::metadata(&path)
            && meta.modified().ok().and_then(|m| m.elapsed().ok()).is_some_and(|age| age < max_age)
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            return Ok(text);
        }
        match self.get_text(url) {
            Ok(text) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let tmp = path.with_extension("tmp");
                if std::fs::write(&tmp, &text).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
                Ok(text)
            }
            Err(e) => std::fs::read_to_string(&path).map_err(|_| e),
        }
    }

    /// Download to a file in the cache, returning its path. Used for icons
    /// the page cannot fetch itself and for release assets.
    pub fn download(&self, url: &str, name: &str) -> Result<PathBuf> {
        let dir = self.cache_dir.join("downloads");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(name);
        let bytes = self.get_bytes(url)?;
        let tmp = path.with_extension("part");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        // A stable file name from the URL: its host plus a hash of the whole.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in url.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        self.cache_dir.join(format!("{}-{h:016x}.json", host(url).replace(['/', ':'], "_")))
    }
}

fn host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
        .to_string()
}

fn describe(url: &str, e: &reqwest::Error) -> String {
    let h = host(url);
    if e.is_timeout() {
        format!("{h} did not answer in time")
    } else if e.is_connect() {
        format!("could not reach {h}")
    } else {
        format!("{h}: {e}")
    }
}
