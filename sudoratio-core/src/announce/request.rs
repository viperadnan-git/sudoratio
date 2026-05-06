//! HTTP GET announce. `Host` is set automatically by reqwest from the URL.

use crate::error::SudoratioError;

pub(crate) async fn send_announce_get(
    http: &reqwest::Client,
    full_url: &str,
    profile_headers: &[(String, String)],
) -> Result<reqwest::Response, SudoratioError> {
    let mut req = http.get(full_url);
    for (k, v) in profile_headers {
        req = req.header(k, v);
    }
    req.send()
        .await
        .map_err(|e| SudoratioError::AnnounceHttp(e.to_string()))
}
