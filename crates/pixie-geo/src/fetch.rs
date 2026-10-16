use std::io::Read;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("Pixie/", env!("CARGO_PKG_VERSION"), " (+https://github.com/pm100/pixie)");

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeoError {
    #[error("network: {0}")]
    Network(String),
    #[error("io: {0}")]
    Io(String),
    #[error("bad tile {0}/{1}/{2}")]
    BadTile(u8, u32, u32),
    /// A newer lookup was requested while this one waited its turn.
    #[error("superseded by a newer lookup")]
    Superseded,
}

/// One blocking HTTP GET. Injected so tests never use the network.
pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, GeoError>;
}

pub struct HttpFetcher {
    agent: ureq::Agent,
}

impl HttpFetcher {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new().user_agent(USER_AGENT).timeout(Duration::from_secs(5)).build();
        HttpFetcher { agent }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, GeoError> {
        let resp = self.agent.get(url).call().map_err(|e| GeoError::Network(e.to_string()))?;
        let mut buf = Vec::new();
        resp.into_reader()
            .take(8 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| GeoError::Network(e.to_string()))?;
        Ok(buf)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    /// Records every call; replies with whatever was last set. URLs containing
    /// "wikipedia.org" get the wiki reply, everything else the main reply.
    pub struct FakeFetcher {
        pub calls: Mutex<Vec<(String, Instant)>>,
        reply: Mutex<Result<Vec<u8>, GeoError>>,
        wiki: Mutex<Result<Vec<u8>, GeoError>>,
    }

    impl FakeFetcher {
        pub fn new(reply: Result<Vec<u8>, GeoError>) -> Self {
            FakeFetcher { calls: Mutex::new(Vec::new()), reply: Mutex::new(reply.clone()), wiki: Mutex::new(reply) }
        }
        pub fn set(&self, reply: Result<Vec<u8>, GeoError>) {
            *self.reply.lock().unwrap() = reply.clone();
            *self.wiki.lock().unwrap() = reply;
        }
        pub fn set_wiki(&self, reply: Result<Vec<u8>, GeoError>) {
            *self.wiki.lock().unwrap() = reply;
        }
        pub fn count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
        pub fn count_matching(&self, needle: &str) -> usize {
            self.calls.lock().unwrap().iter().filter(|(u, _)| u.contains(needle)).count()
        }
    }

    impl Fetcher for FakeFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>, GeoError> {
            self.calls.lock().unwrap().push((url.to_string(), Instant::now()));
            if url.contains("wikipedia.org") {
                self.wiki.lock().unwrap().clone()
            } else {
                self.reply.lock().unwrap().clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_names_the_app() {
        assert!(USER_AGENT.starts_with("Pixie/0.1.0 (+https://github.com/pm100/pixie"));
    }
}
