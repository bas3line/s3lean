#[tokio::main]
async fn main() {
    let client = s3lean::Client::new("http://127.0.0.1:9000", "b", "auto", s3lean::Credentials::new("k", "s")).unwrap();
    let http = reqwest::Client::new();
    let out = client.put("k", b"x".to_vec()).sign(std::time::SystemTime::now()).into_reqwest(&http).send().await;
    println!("{:?}", out.is_ok());
}
