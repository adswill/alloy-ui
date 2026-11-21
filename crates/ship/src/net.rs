//! The only network surface: a tiny trait so apps (and tests) can bring their
//! own HTTP stack, plus a default implementation behind the `http` feature.

#[cfg(feature = "http")]
use crate::Error;
use crate::Result;
use std::io::Write;

/// Fetches and posts bytes.
pub trait Fetch {
    /// GET `url` and return the body.
    fn get(&self, url: &str) -> Result<Vec<u8>>;

    /// GET `url` into `out`. The default buffers; transports that can stream
    /// should override it so large installers do not sit in memory.
    fn download(&self, url: &str, out: &mut dyn Write) -> Result<()> {
        let body = self.get(url)?;
        out.write_all(&body).map_err(crate::io("writing download"))
    }

    /// POST `body` to `url`.
    fn post(&self, url: &str, content_type: &str, body: &[u8]) -> Result<()>;
}

/// HTTP(S) via `ureq` with rustls. Redirects are followed.
#[cfg(feature = "http")]
pub struct UreqFetch {
    agent: ureq::Agent,
}

#[cfg(feature = "http")]
impl Default for UreqFetch {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(600)))
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .user_agent("alloy-ui-ship")
            .build()
            .into();
        Self { agent }
    }
}

#[cfg(feature = "http")]
fn net(e: ureq::Error) -> Error {
    Error::Network(e.to_string())
}

#[cfg(feature = "http")]
impl Fetch for UreqFetch {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let mut resp = self.agent.get(url).call().map_err(net)?;
        // Manifests are small; this bounds a hostile or broken server.
        resp.body_mut()
            .with_config()
            .limit(16 * 1024 * 1024)
            .read_to_vec()
            .map_err(net)
    }

    fn download(&self, url: &str, out: &mut dyn Write) -> Result<()> {
        let mut resp = self.agent.get(url).call().map_err(net)?;
        let mut reader = resp.body_mut().with_config().limit(8 << 30).reader();
        std::io::copy(&mut reader, out)
            .map(|_| ())
            .map_err(crate::io("downloading"))
    }

    fn post(&self, url: &str, content_type: &str, body: &[u8]) -> Result<()> {
        self.agent
            .post(url)
            .header("Content-Type", content_type)
            .send(body)
            .map(|_| ())
            .map_err(net)
    }
}
