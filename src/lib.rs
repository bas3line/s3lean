//! A lean S3 client.
//!
//! Most services that talk to object storage do four things — store an object,
//! fetch one, check one, delete one — and hand out the occasional presigned
//! URL. This crate does those, against Amazon S3 or anything that speaks its
//! API (Cloudflare R2, MinIO, Backblaze B2, Wasabi, DigitalOcean Spaces, …),
//! and nothing else. It brings no HTTP stack of its own: the core signs a
//! request and hands you the method, URL, headers and body, and optional
//! features adapt that to [`reqwest`](https://docs.rs/reqwest) or
//! [`ureq`](https://docs.rs/ureq) if you would rather not do it yourself.
//!
//! ```no_run
//! use std::time::SystemTime;
//! use s3lean::{Client, Credentials};
//!
//! let client = Client::new(
//!     "https://ACCOUNT_ID.r2.cloudflarestorage.com",
//!     "my-bucket",
//!     "auto",
//!     Credentials::new("ACCESS_KEY", "SECRET_KEY"),
//! )?;
//!
//! let request = client
//!     .put("reports/2026-09.json", br#"{"ok":true}"#.to_vec())
//!     .content_type("application/json")
//!     .metadata("source", "nightly")
//!     .sign(SystemTime::now());
//!
//! // `request.method`, `request.url`, `request.headers` and `request.body`
//! // are yours to send with any HTTP client.
//! assert_eq!(request.method, "PUT");
//! # Ok::<(), s3lean::Error>(())
//! ```
//!
//! With the `reqwest` feature, [`SignedRequest::into_reqwest`] does the
//! adapting; with `ureq`, [`SignedRequest::send_ureq`] sends it synchronously:
//!
//! ```ignore
//! let http = reqwest::Client::new();
//! let response = request.into_reqwest(&http).send().await?;
//! assert!(response.status().is_success());
//! ```
//!
//! # What it does not do
//!
//! Multipart uploads, listing, streaming bodies, and credential discovery from
//! the environment or instance metadata. Those are what the SDKs are for; if
//! you need them, use one. Bodies here are `Vec<u8>`, credentials are what you
//! hand in, and an object is one request.
//!
//! # Signing
//!
//! Requests are signed with AWS Signature Version 4 in its S3 form: the object
//! key is encoded segment by segment and not encoded a second time for the
//! canonical request, and the payload hash is sent in `x-amz-content-sha256`.
//! The signer reproduces the worked examples in the S3 documentation exactly
//! (see the tests), and the crate's CI runs every operation against a MinIO
//! server.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod date;
mod encode;
mod error;
mod sign;

#[cfg(feature = "reqwest")]
mod reqwest_transport;
#[cfg(feature = "ureq")]
mod ureq_transport;

use std::time::{Duration, SystemTime};

pub use error::Error;
pub use sign::Credentials;

/// How the bucket appears in the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Addressing {
    /// `https://endpoint/bucket/key`. Works everywhere, needs no DNS for the
    /// bucket, and is what R2 and MinIO serve by default.
    #[default]
    Path,
    /// `https://bucket.endpoint/key`. What Amazon S3 prefers; the bucket name
    /// must be a valid DNS label.
    VirtualHosted,
}

/// A client bound to one bucket at one endpoint.
///
/// It holds no connections and no state beyond its configuration; it builds
/// signed requests. Clone it freely.
#[derive(Debug, Clone)]
pub struct Client {
    scheme: String,
    host: String,
    bucket: String,
    region: String,
    credentials: Credentials,
    addressing: Addressing,
}

impl Client {
    /// `endpoint` is the service root — `https://s3.us-east-1.amazonaws.com`,
    /// `https://ACCOUNT.r2.cloudflarestorage.com`, `http://127.0.0.1:9000` —
    /// with no path. `region` is the signing region: the bucket's region on
    /// Amazon S3, `auto` on R2, whatever the server is configured with on
    /// MinIO (`us-east-1` by default).
    pub fn new(
        endpoint: &str,
        bucket: &str,
        region: &str,
        credentials: Credentials,
    ) -> Result<Self, Error> {
        let (scheme, rest) = endpoint
            .split_once("://")
            .ok_or_else(|| Error::Endpoint(endpoint.to_owned()))?;
        if !matches!(scheme, "http" | "https") {
            return Err(Error::Endpoint(endpoint.to_owned()));
        }
        let host = rest.trim_end_matches('/');
        if host.is_empty() || host.contains('/') || host.contains('?') {
            return Err(Error::Endpoint(endpoint.to_owned()));
        }
        // The signed Host header must be what the client will send, which
        // carries the port only when it is not the scheme's default.
        let host = match (scheme, host.rsplit_once(':')) {
            ("http", Some((bare, "80"))) | ("https", Some((bare, "443"))) => bare,
            _ => host,
        };
        if bucket.is_empty() {
            return Err(Error::Bucket(bucket.to_owned()));
        }
        Ok(Self {
            scheme: scheme.to_owned(),
            host: host.to_owned(),
            bucket: bucket.to_owned(),
            region: region.to_owned(),
            credentials,
            addressing: Addressing::Path,
        })
    }

    /// Selects path or virtual-hosted addressing. The default is path style.
    pub fn addressing(mut self, addressing: Addressing) -> Self {
        self.addressing = addressing;
        self
    }

    /// Stores an object. The payload hash is computed from the body unless
    /// [`Request::payload_sha256`] or [`Request::checksum_sha256`] supplies it.
    ///
    /// An empty key addresses the bucket itself, which is how a bucket is
    /// created: `client.put("", Vec::new())`.
    pub fn put(&self, key: &str, body: impl Into<Vec<u8>>) -> Request<'_> {
        self.request("PUT", key, body.into())
    }

    /// Fetches an object.
    pub fn get(&self, key: &str) -> Request<'_> {
        self.request("GET", key, Vec::new())
    }

    /// Fetches an object's headers — size, ETag, content type and metadata —
    /// without its body. A missing object answers 404.
    pub fn head(&self, key: &str) -> Request<'_> {
        self.request("HEAD", key, Vec::new())
    }

    /// Deletes an object. S3 answers 204 whether or not the object existed.
    pub fn delete(&self, key: &str) -> Request<'_> {
        self.request("DELETE", key, Vec::new())
    }

    fn request(&self, method: &'static str, key: &str, body: Vec<u8>) -> Request<'_> {
        Request {
            client: self,
            method,
            key: key.to_owned(),
            headers: Vec::new(),
            query: Vec::new(),
            body,
            payload_sha256: None,
        }
    }

    fn host_header(&self) -> String {
        match self.addressing {
            Addressing::Path => self.host.clone(),
            Addressing::VirtualHosted => format!("{}.{}", self.bucket, self.host),
        }
    }

    /// The request path, with the key encoded segment by segment. This is both
    /// what goes on the wire and what is signed: S3 encodes the key once.
    fn path(&self, key: &str) -> String {
        let mut path = String::with_capacity(key.len() + self.bucket.len() + 2);
        if self.addressing == Addressing::Path {
            path.push('/');
            path.push_str(&encode::uri(&self.bucket, false));
        }
        path.push('/');
        path.push_str(&encode::uri(key, false));
        path
    }
}

/// One operation, being built. Every header added here is signed.
#[derive(Debug)]
pub struct Request<'c> {
    client: &'c Client,
    method: &'static str,
    key: String,
    headers: Vec<(String, String)>,
    query: Vec<(String, String)>,
    body: Vec<u8>,
    payload_sha256: Option<[u8; 32]>,
}

impl Request<'_> {
    /// Adds a header. It is sent and it is signed, so the server rejects the
    /// request if it is altered in transit.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_owned()));
        self
    }

    /// Adds a query parameter, such as `versionId`.
    pub fn query(mut self, name: &str, value: &str) -> Self {
        self.query.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The `Content-Type` stored with the object.
    pub fn content_type(self, value: &str) -> Self {
        self.header("content-type", value)
    }

    /// The `Content-Encoding` stored with the object, such as `gzip`.
    pub fn content_encoding(self, value: &str) -> Self {
        self.header("content-encoding", value)
    }

    /// User metadata, stored and returned as `x-amz-meta-<name>`. Names are
    /// lowercased, as S3 lowercases them.
    pub fn metadata(self, name: &str, value: &str) -> Self {
        let name = format!("x-amz-meta-{}", name.to_ascii_lowercase());
        self.header(&name, value)
    }

    /// Fetches only the bytes from `start` to `end`, inclusive, of a `get`.
    pub fn range(self, start: u64, end: u64) -> Self {
        let range = format!("bytes={start}-{end}");
        self.header("range", &range)
    }

    /// The body's SHA-256, already computed. It is sent as the signed payload
    /// hash instead of being computed again. Nothing checks that it matches
    /// the body; the server does, by refusing the request.
    pub fn payload_sha256(mut self, digest: [u8; 32]) -> Self {
        self.payload_sha256 = Some(digest);
        self
    }

    /// The body's SHA-256, sent both as the payload hash and as
    /// `x-amz-checksum-sha256`, which asks the server to verify the object
    /// before storing it and to keep the checksum with it.
    pub fn checksum_sha256(self, digest: [u8; 32]) -> Self {
        let encoded = encode::base64(&digest);
        self.payload_sha256(digest)
            .header("x-amz-checksum-sha256", &encoded)
    }

    /// Signs the request as of `at`, which should be the current time: S3
    /// refuses a signature more than fifteen minutes from its own clock.
    pub fn sign(self, at: SystemTime) -> SignedRequest {
        let Request {
            client,
            method,
            key,
            headers,
            query,
            body,
            payload_sha256,
        } = self;
        let path = client.path(&key);
        let payload_hash = match payload_sha256 {
            Some(digest) => encode::hex(&digest),
            None => encode::hex(&sign::sha256(&body)),
        };
        let (mut signed, authorization) = sign::sign_headers(sign::HeaderSigning {
            credentials: &client.credentials,
            region: &client.region,
            method,
            path: &path,
            query: &query,
            host: &client.host_header(),
            headers,
            payload_hash: &payload_hash,
            at,
        });
        signed.push(("authorization".to_owned(), authorization));
        SignedRequest {
            method,
            url: url(client, &path, &query),
            headers: signed,
            body,
        }
    }

    /// A URL that performs this request without credentials until `expires`
    /// after `at`, for handing to a browser or another service. Only the host
    /// is signed, so headers added to the request are not part of it; the
    /// query parameters are.
    ///
    /// S3 caps `expires` at seven days.
    pub fn presign(self, at: SystemTime, expires: Duration) -> String {
        let Request {
            client,
            method,
            key,
            query,
            ..
        } = self;
        let path = client.path(&key);
        let (query, signature) = sign::presign_query(sign::QuerySigning {
            credentials: &client.credentials,
            region: &client.region,
            method,
            path: &path,
            query: &query,
            host: &client.host_header(),
            at,
            expires,
        });
        let mut presigned = url(client, &path, &query);
        presigned.push_str("&X-Amz-Signature=");
        presigned.push_str(&signature);
        presigned
    }
}

fn url(client: &Client, path: &str, query: &[(String, String)]) -> String {
    let mut url = format!("{}://{}{}", client.scheme, client.host_header(), path);
    if !query.is_empty() {
        url.push('?');
        url.push_str(&encode::query(query));
    }
    url
}

/// A request ready to send: everything the wire needs and nothing tied to any
/// HTTP client. `headers` includes `host`, `x-amz-date`,
/// `x-amz-content-sha256` and `authorization`; a client that sets `Host`
/// itself from the URL will set it to the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRequest {
    /// `PUT`, `GET`, `HEAD` or `DELETE`.
    pub method: &'static str,
    /// The full URL, path and query encoded.
    pub url: String,
    /// Lowercase names, in the order they were signed.
    pub headers: Vec<(String, String)>,
    /// Empty for everything but `PUT`.
    pub body: Vec<u8>,
}
