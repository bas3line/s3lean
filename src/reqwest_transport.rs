//! Sending a [`SignedRequest`] with `reqwest`.

use crate::SignedRequest;

impl SignedRequest {
    /// Turns the request into a `reqwest` builder on the given client, ready
    /// for `.send().await`. The builder is returned rather than sent so a
    /// timeout, a trace or a retry policy can be attached first.
    pub fn into_reqwest(self, client: &reqwest::Client) -> reqwest::RequestBuilder {
        let method = reqwest::Method::from_bytes(self.method.as_bytes()).expect("a known method");
        let mut builder = client.request(method, &self.url);
        for (name, value) in self.headers {
            builder = builder.header(name, value);
        }
        if self.method == "PUT" {
            builder = builder.body(self.body);
        }
        builder
    }
}
