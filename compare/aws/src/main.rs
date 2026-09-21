#[tokio::main]
async fn main() {
    let creds = aws_credential_types::Credentials::new("k", "s", None, None, "static");
    let conf = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("auto"))
        .credentials_provider(creds)
        .endpoint_url("http://127.0.0.1:9000")
        .load()
        .await;
    let s3 = aws_sdk_s3::Client::from_conf(aws_sdk_s3::config::Builder::from(&conf).force_path_style(true).build());
    let out = s3.put_object().bucket("b").key("k").body(aws_sdk_s3::primitives::ByteStream::from(b"x".to_vec())).send().await;
    println!("{:?}", out.is_ok());
}
