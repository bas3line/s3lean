//! The worked examples from the S3 documentation on Signature Version 4, with
//! the signatures the documentation prints. A change that alters one byte of
//! the canonical request fails here rather than against a bucket.
//!
//! Source: "Authenticating Requests: Using the Authorization Header
//! (Transferring Payload in a Single Chunk)" and "Authenticating Requests:
//! Using Query Parameters", in the Amazon S3 API reference.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use s3lean::{Addressing, Client, Credentials};

/// The credentials and clock every example uses: 2013-05-24T00:00:00Z.
fn example_client() -> Client {
    Client::new(
        "https://s3.amazonaws.com",
        "examplebucket",
        "us-east-1",
        Credentials::new(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        ),
    )
    .unwrap()
    .addressing(Addressing::VirtualHosted)
}

fn example_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_369_353_600)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
    &headers.iter().find(|(key, _)| key == name).unwrap().1
}

#[test]
fn get_object_example() {
    let request = example_client()
        .get("test.txt")
        .range(0, 9)
        .sign(example_time());

    assert_eq!(
        request.url,
        "https://examplebucket.s3.amazonaws.com/test.txt"
    );
    assert_eq!(
        header(&request.headers, "host"),
        "examplebucket.s3.amazonaws.com"
    );
    assert_eq!(
        header(&request.headers, "x-amz-content-sha256"),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        header(&request.headers, "authorization"),
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
         Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
    );
}

#[test]
fn put_object_example() {
    let request = example_client()
        .put("test$file.text", b"Welcome to Amazon S3.".to_vec())
        .header("date", "Fri, 24 May 2013 00:00:00 GMT")
        .header("x-amz-storage-class", "REDUCED_REDUNDANCY")
        .sign(example_time());

    assert_eq!(
        request.url,
        "https://examplebucket.s3.amazonaws.com/test%24file.text"
    );
    assert_eq!(
        header(&request.headers, "x-amz-content-sha256"),
        "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072"
    );
    assert_eq!(
        header(&request.headers, "authorization"),
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=date;host;x-amz-content-sha256;x-amz-date;x-amz-storage-class, \
         Signature=98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd"
    );
    assert_eq!(request.body, b"Welcome to Amazon S3.");
}

#[test]
fn presigned_get_object_example() {
    let url = example_client()
        .get("test.txt")
        .presign(example_time(), Duration::from_secs(86_400));

    assert_eq!(
        url,
        "https://examplebucket.s3.amazonaws.com/test.txt\
         ?X-Amz-Algorithm=AWS4-HMAC-SHA256\
         &X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request\
         &X-Amz-Date=20130524T000000Z\
         &X-Amz-Expires=86400\
         &X-Amz-SignedHeaders=host\
         &X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
    );
}

#[test]
fn path_style_puts_the_bucket_in_the_path() {
    let client = Client::new(
        "http://127.0.0.1:9000",
        "my-bucket",
        "us-east-1",
        Credentials::new("k", "s"),
    )
    .unwrap();
    let request = client.get("dir/a=b.txt").sign(example_time());
    assert_eq!(request.url, "http://127.0.0.1:9000/my-bucket/dir/a%3Db.txt");
    assert_eq!(header(&request.headers, "host"), "127.0.0.1:9000");
}

#[test]
fn an_empty_key_addresses_the_bucket() {
    let request = example_client().put("", Vec::new()).sign(example_time());
    assert_eq!(request.url, "https://examplebucket.s3.amazonaws.com/");
    let path_style = Client::new("https://s3.example", "b", "r", Credentials::new("k", "s"))
        .unwrap()
        .delete("")
        .sign(example_time());
    assert_eq!(path_style.url, "https://s3.example/b/");
}

#[test]
fn a_session_token_is_sent_and_signed() {
    let client = Client::new(
        "https://s3.example",
        "b",
        "r",
        Credentials::new("k", "s").with_session_token("token-123"),
    )
    .unwrap();
    let request = client.head("x").sign(example_time());
    assert_eq!(
        header(&request.headers, "x-amz-security-token"),
        "token-123"
    );
    assert!(header(&request.headers, "authorization").contains("x-amz-security-token"));
    let url = client
        .get("x")
        .presign(example_time(), Duration::from_secs(60));
    assert!(url.contains("X-Amz-Security-Token=token-123"));
}

#[test]
fn default_ports_are_dropped_from_the_host_and_others_kept() {
    let dropped = Client::new(
        "https://s3.example:443",
        "b",
        "r",
        Credentials::new("k", "s"),
    )
    .unwrap();
    assert_eq!(
        header(&dropped.head("x").sign(example_time()).headers, "host"),
        "s3.example"
    );
    let kept = Client::new(
        "https://s3.example:8443",
        "b",
        "r",
        Credentials::new("k", "s"),
    )
    .unwrap();
    assert_eq!(
        header(&kept.head("x").sign(example_time()).headers, "host"),
        "s3.example:8443"
    );
}

#[test]
fn bad_endpoints_are_refused() {
    for endpoint in [
        "s3.example",
        "ftp://s3.example",
        "https://",
        "https://s3.example/bucket",
    ] {
        assert!(
            Client::new(endpoint, "b", "r", Credentials::new("k", "s")).is_err(),
            "{endpoint}"
        );
    }
    assert!(Client::new("https://s3.example", "", "r", Credentials::new("k", "s")).is_err());
}

#[test]
fn checksum_sets_both_the_payload_hash_and_the_checksum_header() {
    use sha2::Digest;
    let body = b"Welcome to Amazon S3.".to_vec();
    let digest: [u8; 32] = sha2::Sha256::digest(&body).into();
    let request = example_client()
        .put("k", body)
        .checksum_sha256(digest)
        .sign(example_time());
    assert_eq!(
        header(&request.headers, "x-amz-content-sha256"),
        "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072"
    );
    assert_eq!(
        header(&request.headers, "x-amz-checksum-sha256"),
        "RM591nyVng01JP+sF3Hfu6h9K2tLTpnkIDSouAP4sHI="
    );
}
