//! Cross-platform HTTP request dispatcher
//! Desktop: Multi-threaded ureq with native TLS and OS sockets
//! WebAssembly: ehttp with browser fetch() API

use crate::gis::source::RequestHandle;

pub fn fetch_bytes_cancellable(
    url: &str,
    on_done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) -> RequestHandle {
    let (handle, cancelled) = RequestHandle::new();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let url_owned = url.to_string();
        let flag = cancelled;
        std::thread::spawn(move || {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let res = fetch_bytes_sync(&url_owned);
            if !flag.load(std::sync::atomic::Ordering::Relaxed) {
                on_done(res);
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        let url_owned = url.to_string();
        let request = ehttp::Request::get(url);
        let flag = cancelled;
        ehttp::fetch(request, move |response| {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let res = match response {
                Ok(resp) => {
                    if resp.ok {
                        log::debug!("[ehttp] 200 OK ({} bytes) from {}", resp.bytes.len(), url_owned);
                        Ok(resp.bytes)
                    } else {
                        log::warn!("[ehttp] HTTP {} {} from {}", resp.status, resp.status_text, url_owned);
                        Err(format!("HTTP status {}: {}", resp.status, resp.status_text))
                    }
                }
                Err(err) => {
                    log::warn!("[ehttp] Error requesting {}: {}", url_owned, err);
                    Err(err)
                }
            };
            on_done(res);
        });
    }

    handle
}

pub fn fetch_bytes(
    url: &str,
    on_done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    let _ = fetch_bytes_cancellable(url, on_done);
}

pub fn fetch_text(
    url: &str,
    on_done: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    fetch_bytes(url, move |res| {
        let text_res = res.and_then(|bytes| {
            String::from_utf8(bytes).map_err(|e| format!("UTF-8 decode error: {}", e))
        });
        on_done(text_res);
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_bytes_sync(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(15))
        .user_agent("s3d-gis/0.1.0 (Antigravity 3D GIS & Solar Analysis Engine)")
        .build();

    let resp = agent.get(url).call().map_err(|e| format!("HTTP request error: {}", e))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes)
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    Ok(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_text_sync(url: &str) -> Result<String, String> {
    let bytes = fetch_bytes_sync(url)?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 decode error: {}", e))
}
