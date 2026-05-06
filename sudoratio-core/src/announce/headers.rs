//! Build the per-announce HTTP headers from the active profile, resolving `{java}`, `{os}` and
//! `{locale}` placeholders against the process environment.

use crate::profile::ClientProfileSpec;

pub fn resolve_header_value(value: &str) -> String {
    let mut v = value.to_string();
    if let Ok(java) = std::env::var("JAVA_VERSION") {
        v = v.replace("{java}", &java);
    } else {
        v = v.replace("{java}", "21");
    }
    if let Ok(os) = std::env::var("OS_NAME") {
        v = v.replace("{os}", &os);
    } else {
        v = v.replace("{os}", std::env::consts::OS);
    }
    if let Ok(loc) = std::env::var("LOCALE") {
        v = v.replace("{locale}", &loc);
    } else {
        v = v.replace(
            "{locale}",
            &std::env::var("LANG").unwrap_or_else(|_| "en-US".into()),
        );
    }
    let re = regex::Regex::new(r"\{[^}]+\}").unwrap();
    if re.is_match(&v) {
        return v;
    }
    v
}

pub fn build_request_headers(client: &ClientProfileSpec) -> Vec<(String, String)> {
    client
        .request_headers
        .iter()
        .map(|h| (h.name.clone(), resolve_header_value(&h.value)))
        .collect()
}
