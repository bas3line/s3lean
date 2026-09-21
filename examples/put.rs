//! Uploads a file with `reqwest`, then reads its headers back.
//!
//!     S3LEAN_ENDPOINT=http://127.0.0.1:9000 S3LEAN_BUCKET=demo \
//!     S3LEAN_REGION=us-east-1 S3LEAN_ACCESS_KEY=minioadmin S3LEAN_SECRET_KEY=minioadmin \
//!     cargo run --example put --features reqwest -- ./README.md docs/readme.md

use std::time::SystemTime;

use s3lean::{Client, Credentials};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (file, key) = (
        args.next().ok_or("usage: put <file> <key>")?,
        args.next().ok_or("usage: put <file> <key>")?,
    );
    let env = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));

    let client = Client::new(
        &env("S3LEAN_ENDPOINT")?,
        &env("S3LEAN_BUCKET")?,
        &env("S3LEAN_REGION")?,
        Credentials::new(&env("S3LEAN_ACCESS_KEY")?, &env("S3LEAN_SECRET_KEY")?),
    )?;
    let http = reqwest::Client::new();

    let body = std::fs::read(&file)?;
    let stored = client
        .put(&key, body)
        .content_type("application/octet-stream")
        .metadata("uploaded-from", &file)
        .sign(SystemTime::now())
        .into_reqwest(&http)
        .send()
        .await?;
    println!("put {key}: {}", stored.status());

    let head = client
        .head(&key)
        .sign(SystemTime::now())
        .into_reqwest(&http)
        .send()
        .await?;
    println!("head {key}: {}", head.status());
    for name in [
        "content-length",
        "content-type",
        "etag",
        "x-amz-meta-uploaded-from",
    ] {
        if let Some(value) = head.headers().get(name) {
            println!("  {name}: {}", value.to_str().unwrap_or("?"));
        }
    }
    Ok(())
}
