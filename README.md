# s3lean

[![crates.io](https://img.shields.io/crates/v/s3lean.svg)](https://crates.io/crates/s3lean)
[![docs.rs](https://docs.rs/s3lean/badge.svg)](https://docs.rs/s3lean)
[![ci](https://github.com/bas3line/s3lean/actions/workflows/ci.yml/badge.svg)](https://github.com/bas3line/s3lean/actions/workflows/ci.yml)

A lean S3 client for Rust. Signed `PutObject`, `GetObject`, `HeadObject`,
`DeleteObject` and presigned URLs — on the HTTP client you already have.

Works with Amazon S3 and everything that speaks its API: Cloudflare R2, MinIO,
Backblaze B2, Wasabi, DigitalOcean Spaces, Garage, and the rest.

## Why

Most services that touch object storage do four things: store an object,
fetch one, check one, delete one. The official SDK does those and several
hundred other things, and you pay for all of them every time you build.

| | `aws-sdk-s3` + `aws-config` | `s3lean` + `reqwest` | `s3lean` alone |
| --- | ---: | ---: | ---: |
| Crates in the build | 207 | 126 | 11 |
| Release binary, stripped, thin LTO | 6.5 MiB | 2.6 MiB | — |
| Clean release build, 10-core arm64 | 69 s | 13 s | — |

Both columns are a program that signs and sends one `PutObject` on tokio with
rustls, with the SDK configured as lean as it gets (`default-features = false`
plus `rustls` and `rt-tokio`). The projects are in [`compare/`](compare/);
run them yourself. Of `s3lean`'s eleven crates, every one is the SHA-256 and
HMAC stack — there is no HTTP client, no TLS, no runtime and no
serialisation in the core, because signing a request needs none of them. The
other 115 in the middle column are `reqwest`'s, which you were going to build
anyway.

## Use

```toml
[dependencies]
s3lean = { version = "0.1", features = ["reqwest"] }
```

```rust
use std::time::SystemTime;
use s3lean::{Client, Credentials};

let client = Client::new(
    "https://ACCOUNT_ID.r2.cloudflarestorage.com",
    "my-bucket",
    "auto",
    Credentials::new("ACCESS_KEY", "SECRET_KEY"),
)?;
let http = reqwest::Client::new();

// Store
let stored = client
    .put("reports/2026-09.json", body)
    .content_type("application/json")
    .metadata("source", "nightly")
    .sign(SystemTime::now())
    .into_reqwest(&http)
    .send()
    .await?;
assert!(stored.status().is_success());

// Fetch, in whole or in part
let bytes = client.get("reports/2026-09.json").sign(SystemTime::now())
    .into_reqwest(&http).send().await?.bytes().await?;
let first_kb = client.get("reports/2026-09.json").range(0, 1023).sign(SystemTime::now())
    .into_reqwest(&http).send().await?;

// Check, delete
let head = client.head("reports/2026-09.json").sign(SystemTime::now()).into_reqwest(&http).send().await?;
let gone = client.delete("reports/2026-09.json").sign(SystemTime::now()).into_reqwest(&http).send().await?;

// Hand someone a link that works for an hour and needs no credentials
let url = client.get("reports/2026-09.json").presign(SystemTime::now(), Duration::from_secs(3600));
```

### Without a feature flag

The core produces a [`SignedRequest`](https://docs.rs/s3lean/latest/s3lean/struct.SignedRequest.html)
— method, URL, headers, body — and you send it with whatever you like:

```rust
let request = client.put("k", body).sign(SystemTime::now());
// request.method == "PUT"
// request.url    == "https://…/my-bucket/k"
// request.headers: host, x-amz-date, x-amz-content-sha256, authorization, …
// request.body
```

### Synchronously, with `ureq`

```toml
s3lean = { version = "0.1", features = ["ureq"] }
```

```rust
let agent = ureq::agent();
let response = client.put("k", body).sign(SystemTime::now()).send_ureq(&agent)?;
```

### Temporary credentials

```rust
Credentials::new(access_key, secret_key).with_session_token(token)
```

The token is sent as `x-amz-security-token` and signed, and rides along in
presigned URLs.

### Checksums

If you already have the body's SHA-256 — because you named the object by it,
say — hand it over and it is not computed again:

```rust
client.put(&key, body).checksum_sha256(digest)   // also sets x-amz-checksum-sha256
client.put(&key, body).payload_sha256(digest)    // signs with it, no checksum header
```

### Addressing

Path style (`https://endpoint/bucket/key`) is the default; it needs no DNS for
the bucket and is what R2 and MinIO serve. Amazon S3 prefers the bucket in the
host name:

```rust
Client::new("https://s3.eu-west-1.amazonaws.com", "bucket", "eu-west-1", creds)?
    .addressing(Addressing::VirtualHosted)
```

### Buckets

An empty key addresses the bucket itself, so creating one is a `put`:

```rust
client.put("", Vec::new()).sign(SystemTime::now())
```

## What it does not do

Multipart uploads, listing, streaming bodies, and finding credentials in the
environment or instance metadata. Those are what the SDKs are for. Bodies are
`Vec<u8>`, credentials are what you hand in, and an object is one request. If
your objects are bigger than you want in memory, this is the wrong crate.

## How you know it is right

The signer reproduces the worked examples in the S3 documentation exactly —
the `GET`, the `PUT` with a key that needs encoding, and the presigned URL —
with the signatures the documentation prints, in [`tests/vectors.rs`](tests/vectors.rs).
A change that alters one byte of the canonical request fails there rather
than against your bucket.

CI then runs every operation against a real MinIO server: create the bucket,
put with metadata and a checksum, head, get, ranged get, fetch through a
presigned URL, delete, and confirm it is gone. Point
[`tests/live.rs`](tests/live.rs) at your own bucket with `S3LEAN_ENDPOINT`,
`S3LEAN_BUCKET`, `S3LEAN_REGION`, `S3LEAN_ACCESS_KEY` and `S3LEAN_SECRET_KEY`
to run the same against it.

## Features

| Feature | Adds |
| --- | --- |
| `reqwest` | `SignedRequest::into_reqwest`, for async sending |
| `ureq` | `SignedRequest::send_ureq`, for synchronous sending |

Neither is on by default. The core has no HTTP dependency.

## Minimum supported Rust

1.75.

## License

MIT or Apache-2.0, at your option.
