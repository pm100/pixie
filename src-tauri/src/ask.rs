//! Ask Pixie's one network call: the typed question and some app information go to the proxy (ask-proxy/), the answer comes
//! back. Pictures are never part of it: the body is JSON text made by the UI from typed words and settings, and it is
//! refused here if it is large enough to hold anything else.

use std::io::Read;
use std::time::Duration;

/// The most the proxy accepts (ask-proxy/src/validate.ts MAX_BODY_BYTES).
const MAX_BODY: usize = 32_768;
/// A reply is a few sentences; anything bigger is not from the proxy.
const MAX_REPLY: u64 = 64 * 1024;

/// What came back: the HTTP status (0: no connection, -1: this build has no address for the proxy) and the body text.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Answer {
    pub status: i32,
    pub body: String,
}

/// The proxy's address from the build or the environment, if it is one we may talk to: https, or http to this computer
/// (for `wrangler dev`).
pub fn endpoint(configured: Option<&str>) -> Option<String> {
    let url = configured?.trim();
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    (url.starts_with("https://") || local).then(|| url.to_string())
}

/// Where the proxy is deployed (ask-proxy/). The address is public: what protects it is the daily limit per install.
const DEFAULT_ENDPOINT: &str = "https://pixie-ask.paulmoore100.workers.dev/v1/ask";

/// The proxy's address: `PIXIE_ASK_URL` when it runs (for trying `wrangler dev`), or when Pixie was built, else the deployed one.
pub fn configured_endpoint() -> Option<String> {
    endpoint(std::env::var("PIXIE_ASK_URL").ok().as_deref())
        .or_else(|| endpoint(option_env!("PIXIE_ASK_URL")))
        .or_else(|| endpoint(Some(DEFAULT_ENDPOINT)))
}

/// The install id is a random UUID the UI made; nothing else is put in a header.
fn is_install_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// A dev key is 32 hex digits (nothing else is ever put in a header).
fn is_dev_key(key: &str) -> bool {
    key.len() == 32 && key.chars().all(|c| c.is_ascii_hexdigit())
}

/// The key that makes this a dev build as far as the proxy is concerned (it is not limited to a few questions a day). Only a
/// debug build has one, so a release build never sends it, whatever is on the computer: from `PIXIE_ASK_DEV_KEY` when it runs,
/// else from the file `ask-dev-key.txt` in Pixie's data folder. The proxy holds the same key as a secret and decides.
pub fn dev_key(debug: bool, from_env: Option<&str>, file: &std::path::Path) -> Option<String> {
    if !debug {
        return None;
    }
    let valid = |s: &str| is_dev_key(s.trim()).then(|| s.trim().to_string());
    from_env.and_then(valid).or_else(|| std::fs::read_to_string(file).ok().and_then(|t| valid(&t)))
}

/// Sends one question. A status the proxy answered with (429 for the day's limit, 502 when the model failed) comes back
/// as an answer, not an error, so the UI can say the right thing. `dev_key`: see `dev_key`.
pub fn post(url: &str, install_id: &str, body: &str, dev_key: Option<&str>) -> Answer {
    if !is_install_id(install_id) || body.len() > MAX_BODY {
        return Answer { status: 400, body: String::new() };
    }
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build();
    let mut request = agent.post(url).set("Content-Type", "application/json").set("X-Install-Id", install_id);
    if let Some(key) = dev_key.filter(|k| is_dev_key(k)) {
        request = request.set("X-Dev-Key", key);
    }
    let result = request.send_string(body);
    let (status, response) = match result {
        Ok(r) => (r.status(), r),
        Err(ureq::Error::Status(code, r)) => (code, r),
        Err(_) => return Answer { status: 0, body: String::new() },
    };
    let mut text = String::new();
    // Over the limit, or not text: leave the body empty rather than pass on something unexpected.
    if response.into_reader().take(MAX_REPLY).read_to_string(&mut text).is_err() {
        text.clear();
    }
    Answer { status: i32::from(status), body: text }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const ID: &str = "123e4567-e89b-42d3-a456-426614174000";

    /// A one-shot server on this computer: answers the first request with `status` and `body`, and reports what it received.
    fn serve(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1/ask", listener.local_addr().unwrap().port());
        let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                seen.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&seen).to_string();
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let len = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if seen.len() >= head_end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&seen).to_string()
        });
        (url, handle)
    }

    #[test]
    fn only_https_or_this_computer_may_be_asked() {
        assert_eq!(endpoint(Some("https://pixie-ask.example.workers.dev/v1/ask")).as_deref(), Some("https://pixie-ask.example.workers.dev/v1/ask"));
        assert_eq!(endpoint(Some(" http://localhost:8787/v1/ask ")).as_deref(), Some("http://localhost:8787/v1/ask"));
        assert!(endpoint(Some("http://127.0.0.1:8787/v1/ask")).is_some());
        assert!(endpoint(Some("http://example.com/v1/ask")).is_none());
        assert!(endpoint(Some("ftp://example.com")).is_none());
        assert!(endpoint(Some("")).is_none());
        assert!(endpoint(None).is_none());
    }

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_dev_key_is_sent_in_its_header() {
        let (url, server) = serve("200 OK", "{}");
        post(&url, ID, "{}", Some(KEY));
        assert!(server.join().unwrap().to_ascii_lowercase().contains(&format!("x-dev-key: {KEY}")));
    }

    #[test]
    fn nothing_is_sent_for_a_key_that_is_not_hex_of_the_right_length() {
        for bad in ["short", "0123456789abcdef0123456789abcdeg", "0123456789abcdef0123456789abcdef
X: y"] {
            let (url, server) = serve("200 OK", "{}");
            post(&url, ID, "{}", Some(bad));
            assert!(!server.join().unwrap().to_ascii_lowercase().contains("x-dev-key"), "{bad:?}");
        }
    }

    #[test]
    fn only_a_debug_build_has_a_dev_key() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ask-dev-key.txt");
        std::fs::write(&file, format!("{KEY}
")).unwrap();
        assert_eq!(dev_key(true, None, &file).as_deref(), Some(KEY));
        assert_eq!(dev_key(false, Some(KEY), &file), None);
    }

    #[test]
    fn the_environment_wins_over_the_file_and_a_bad_value_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ask-dev-key.txt");
        std::fs::write(&file, KEY).unwrap();
        let other = "fedcba9876543210fedcba9876543210";
        assert_eq!(dev_key(true, Some(other), &file).as_deref(), Some(other));
        assert_eq!(dev_key(true, Some("nonsense"), &file).as_deref(), Some(KEY));
        assert_eq!(dev_key(true, None, &dir.path().join("missing.txt")), None);
    }

    #[test]
    fn the_deployed_address_is_one_we_may_talk_to() {
        assert!(endpoint(Some(DEFAULT_ENDPOINT)).is_some());
    }

    #[test]
    fn sends_the_body_and_the_install_id_and_returns_the_answer() {
        let (url, server) = serve("200 OK", r#"{"reply":"Hi."}"#);
        let answer = post(&url, ID, r#"{"messages":[]}"#, None);
        assert_eq!(answer, Answer { status: 200, body: r#"{"reply":"Hi."}"#.to_string() });
        let seen = server.join().unwrap().to_ascii_lowercase();
        assert!(seen.starts_with("post /v1/ask"));
        assert!(seen.contains(&format!("x-install-id: {ID}")));
        assert!(seen.contains("content-type: application/json"));
        assert!(seen.ends_with(r#"{"messages":[]}"#));
    }

    #[test]
    fn an_answer_with_an_error_status_still_comes_back_with_its_body() {
        let (url, server) = serve("429 Too Many Requests", r#"{"error":"quota_exceeded"}"#);
        let answer = post(&url, ID, "{}", None);
        server.join().unwrap();
        assert_eq!(answer.status, 429);
        assert!(answer.body.contains("quota_exceeded"));
    }

    #[test]
    fn no_connection_is_status_zero() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1/ask", listener.local_addr().unwrap().port());
        drop(listener);
        assert_eq!(post(&url, ID, "{}", None), Answer { status: 0, body: String::new() });
    }

    #[test]
    fn refuses_a_bad_install_id_or_a_body_too_big_to_be_a_question() {
        assert_eq!(post("http://127.0.0.1:9/", "x\r\nHost: evil", "{}", None).status, 400);
        assert_eq!(post("http://127.0.0.1:9/", ID, &"x".repeat(MAX_BODY + 1), None).status, 400);
    }
}
