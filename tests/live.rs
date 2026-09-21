//! Every operation, against a real server.
//!
//! Runs when `S3LEAN_ENDPOINT` is set, against whatever it names — a MinIO
//! container in CI, or a real bucket on S3, R2 or elsewhere — with
//! `S3LEAN_BUCKET`, `S3LEAN_REGION`, `S3LEAN_ACCESS_KEY` and
//! `S3LEAN_SECRET_KEY`. Set `S3LEAN_VIRTUAL_HOSTED=1` for a server that wants
//! the bucket in the host name. The bucket is created if it does not exist and
//! is left in place; every object the test writes is deleted.
//!
//! Skipped, not failed, when the variables are absent, so `cargo test` stays
//! green offline.

#![cfg(feature = "reqwest")]

use std::time::{Duration, SystemTime};

use s3lean::{Addressing, Client, Credentials};

fn client_from_env() -> Option<Client> {
    let endpoint = std::env::var("S3LEAN_ENDPOINT").ok()?;
    let bucket = std::env::var("S3LEAN_BUCKET").unwrap_or_else(|_| "s3lean-test".into());
    let region = std::env::var("S3LEAN_REGION").unwrap_or_else(|_| "us-east-1".into());
    let access = std::env::var("S3LEAN_ACCESS_KEY").expect("S3LEAN_ACCESS_KEY");
    let secret = std::env::var("S3LEAN_SECRET_KEY").expect("S3LEAN_SECRET_KEY");
    let mut client = Client::new(
        &endpoint,
        &bucket,
        &region,
        Credentials::new(&access, &secret),
    )
    .unwrap();
    if std::env::var("S3LEAN_VIRTUAL_HOSTED").is_ok() {
        client = client.addressing(Addressing::VirtualHosted);
    }
    Some(client)
}

#[tokio::test]
async fn put_head_get_presign_delete_round_trip() {
    let Some(client) = client_from_env() else {
        eprintln!("S3LEAN_ENDPOINT not set; skipping the live test");
        return;
    };
    let http = reqwest::Client::new();
    let now = SystemTime::now;

    // Create the bucket; an existing one answers 409 on MinIO and S3 alike.
    let created = client
        .put("", Vec::new())
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert!(
        created.status().is_success() || created.status() == 409,
        "create bucket: {} {}",
        created.status(),
        created.text().await.unwrap_or_default()
    );

    let key = format!(
        "s3lean/{}/a=b c~d.json",
        now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let body = br#"{"hello":"world","n":42}"#.to_vec();
    let digest: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&body).into();

    let put = client
        .put(&key, body.clone())
        .content_type("application/json")
        .content_encoding("identity")
        .metadata("Source", "live-test")
        .checksum_sha256(digest)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert!(
        put.status().is_success(),
        "put: {} {}",
        put.status(),
        put.text().await.unwrap_or_default()
    );

    let head = client
        .head(&key)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200, "head");
    assert_eq!(head.headers()["content-type"], "application/json");
    assert_eq!(head.headers()["x-amz-meta-source"], "live-test");
    assert_eq!(
        head.headers()["content-length"],
        body.len().to_string().as_str()
    );

    let get = client
        .get(&key)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 200, "get");
    assert_eq!(get.bytes().await.unwrap().as_ref(), &body[..]);

    let partial = client
        .get(&key)
        .range(2, 8)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), 206, "ranged get");
    assert_eq!(partial.bytes().await.unwrap().as_ref(), &body[2..=8]);

    let url = client.get(&key).presign(now(), Duration::from_secs(300));
    let presigned = http.get(&url).send().await.unwrap();
    assert_eq!(presigned.status(), 200, "presigned get: {url}");
    assert_eq!(presigned.bytes().await.unwrap().as_ref(), &body[..]);

    let deleted = client
        .delete(&key)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), 204, "delete");

    let gone = client
        .head(&key)
        .sign(now())
        .into_reqwest(&http)
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), 404, "head after delete");
}

#[cfg(feature = "ureq")]
#[test]
fn the_sync_transport_round_trips_too() {
    let Some(client) = client_from_env() else {
        return;
    };
    let agent = ureq::agent();
    let key = "s3lean/ureq.txt";
    let body = b"sync".to_vec();

    let put = client
        .put(key, body.clone())
        .content_type("text/plain")
        .sign(SystemTime::now())
        .send_ureq(&agent)
        .unwrap();
    assert!(put.status().is_success());

    let mut got = client
        .get(key)
        .sign(SystemTime::now())
        .send_ureq(&agent)
        .unwrap();
    assert_eq!(got.body_mut().read_to_vec().unwrap(), body);

    let deleted = client
        .delete(key)
        .sign(SystemTime::now())
        .send_ureq(&agent)
        .unwrap();
    assert_eq!(deleted.status(), 204);
}
