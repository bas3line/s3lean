//! AWS Signature Version 4, S3 flavour.
//!
//! Two forms: headers, where the signature travels in `Authorization` and
//! covers every header the request sends, and query, where it travels in the
//! URL and covers only the host, for presigned links. Both hash the same
//! canonical request and derive the same signing key.

use std::time::{Duration, SystemTime};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::date::amz_date;
use crate::encode;

/// An access key pair, with the session token that temporary credentials
/// carry.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
}

impl Credentials {
    /// Long-lived credentials: an access key id and its secret.
    pub fn new(access_key: &str, secret_key: &str) -> Self {
        Self {
            access_key: access_key.to_owned(),
            secret_key: secret_key.to_owned(),
            session_token: None,
        }
    }

    /// Adds the session token that STS or an instance role issues alongside
    /// temporary keys. It is sent as `x-amz-security-token` and signed.
    pub fn with_session_token(mut self, token: &str) -> Self {
        self.session_token = Some(token.to_owned());
        self
    }
}

/// The secret is deliberately not printed.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key", &self.access_key)
            .field("session_token", &self.session_token.as_ref().map(|_| "…"))
            .finish_non_exhaustive()
    }
}

pub(crate) struct HeaderSigning<'a> {
    pub credentials: &'a Credentials,
    pub region: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a [(String, String)],
    pub host: &'a str,
    pub headers: Vec<(String, String)>,
    pub payload_hash: &'a str,
    pub at: SystemTime,
}

/// Returns the headers to send — the caller's, plus `host`, `x-amz-date`,
/// `x-amz-content-sha256` and the session token — and the `Authorization`
/// value that signs all of them.
pub(crate) fn sign_headers(input: HeaderSigning<'_>) -> (Vec<(String, String)>, String) {
    let (date, day) = amz_date(input.at);
    let mut headers = input.headers;
    headers.push(("host".to_owned(), input.host.to_owned()));
    headers.push((
        "x-amz-content-sha256".to_owned(),
        input.payload_hash.to_owned(),
    ));
    headers.push(("x-amz-date".to_owned(), date.clone()));
    if let Some(token) = &input.credentials.session_token {
        headers.push(("x-amz-security-token".to_owned(), token.clone()));
    }
    for (name, value) in &mut headers {
        name.make_ascii_lowercase();
        *value = canonical_value(value);
    }
    headers.sort();

    let signed_headers = headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical = canonical_request(
        input.method,
        input.path,
        &encode::query(input.query),
        &headers,
        &signed_headers,
        input.payload_hash,
    );
    let scope = format!("{day}/{}/s3/aws4_request", input.region);
    let signature = signature(
        input.credentials,
        &day,
        input.region,
        &date,
        &scope,
        &canonical,
    );
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        input.credentials.access_key
    );
    (headers, authorization)
}

pub(crate) struct QuerySigning<'a> {
    pub credentials: &'a Credentials,
    pub region: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a [(String, String)],
    pub host: &'a str,
    pub at: SystemTime,
    pub expires: Duration,
}

/// Returns the query parameters of a presigned URL — the caller's plus the
/// `X-Amz-*` ones — and the signature over them, kept apart so the URL can
/// carry it last, where every S3 tool puts it.
pub(crate) fn presign_query(input: QuerySigning<'_>) -> (Vec<(String, String)>, String) {
    let (date, day) = amz_date(input.at);
    let scope = format!("{day}/{}/s3/aws4_request", input.region);
    let mut query: Vec<(String, String)> = input.query.to_vec();
    query.push(("X-Amz-Algorithm".into(), "AWS4-HMAC-SHA256".into()));
    query.push((
        "X-Amz-Credential".into(),
        format!("{}/{scope}", input.credentials.access_key),
    ));
    query.push(("X-Amz-Date".into(), date.clone()));
    query.push(("X-Amz-Expires".into(), input.expires.as_secs().to_string()));
    query.push(("X-Amz-SignedHeaders".into(), "host".into()));
    if let Some(token) = &input.credentials.session_token {
        query.push(("X-Amz-Security-Token".into(), token.clone()));
    }
    let headers = vec![("host".to_owned(), input.host.to_owned())];
    let canonical = canonical_request(
        input.method,
        input.path,
        &encode::query(&query),
        &headers,
        "host",
        "UNSIGNED-PAYLOAD",
    );
    let signature = signature(
        input.credentials,
        &day,
        input.region,
        &date,
        &scope,
        &canonical,
    );
    (query, signature)
}

fn canonical_request(
    method: &str,
    path: &str,
    query: &str,
    headers: &[(String, String)],
    signed_headers: &str,
    payload_hash: &str,
) -> String {
    let mut canonical = String::with_capacity(256 + headers.len() * 48);
    canonical.push_str(method);
    canonical.push('\n');
    canonical.push_str(path);
    canonical.push('\n');
    canonical.push_str(query);
    canonical.push('\n');
    for (name, value) in headers {
        canonical.push_str(name);
        canonical.push(':');
        canonical.push_str(value);
        canonical.push('\n');
    }
    canonical.push('\n');
    canonical.push_str(signed_headers);
    canonical.push('\n');
    canonical.push_str(payload_hash);
    canonical
}

fn signature(
    credentials: &Credentials,
    day: &str,
    region: &str,
    date: &str,
    scope: &str,
    canonical: &str,
) -> String {
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{date}\n{scope}\n{}",
        encode::hex(&sha256(canonical.as_bytes()))
    );
    let mut key = hmac(
        format!("AWS4{}", credentials.secret_key).as_bytes(),
        day.as_bytes(),
    );
    for part in [region, "s3", "aws4_request"] {
        key = hmac(&key, part.as_bytes());
    }
    encode::hex(&hmac(&key, string_to_sign.as_bytes()))
}

/// A header value as the canonical request wants it: trimmed, with runs of
/// spaces collapsed to one.
fn canonical_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_space = false;
    for ch in value.trim().chars() {
        if ch == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out
}

pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_values_are_trimmed_and_collapsed() {
        assert_eq!(canonical_value("  a   b  c "), "a b c");
        assert_eq!(canonical_value("plain"), "plain");
    }

    #[test]
    fn credentials_do_not_print_their_secret() {
        let creds = Credentials::new("AKIA", "very-secret").with_session_token("FQoGZXIvYXdz");
        let printed = format!("{creds:?}");
        assert!(printed.contains("AKIA"));
        assert!(!printed.contains("very-secret"));
        assert!(!printed.contains("FQoGZXIvYXdz"));
    }
}
