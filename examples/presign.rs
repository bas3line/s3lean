//! Prints a presigned URL for downloading an object, valid for an hour. Needs
//! no HTTP client at all: the URL is the whole product.
//!
//!     S3LEAN_ENDPOINT=https://ACCOUNT.r2.cloudflarestorage.com \
//!     S3LEAN_BUCKET=my-bucket S3LEAN_REGION=auto \
//!     S3LEAN_ACCESS_KEY=… S3LEAN_SECRET_KEY=… \
//!     cargo run --example presign -- path/to/object.pdf

use std::time::{Duration, SystemTime};

use s3lean::{Client, Credentials};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::args().nth(1).ok_or("usage: presign <key>")?;
    let env = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));

    let client = Client::new(
        &env("S3LEAN_ENDPOINT")?,
        &env("S3LEAN_BUCKET")?,
        &env("S3LEAN_REGION")?,
        Credentials::new(&env("S3LEAN_ACCESS_KEY")?, &env("S3LEAN_SECRET_KEY")?),
    )?;

    println!(
        "{}",
        client
            .get(&key)
            .presign(SystemTime::now(), Duration::from_secs(3600))
    );
    Ok(())
}
