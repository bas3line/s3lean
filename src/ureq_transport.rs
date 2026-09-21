//! Sending a [`SignedRequest`] with `ureq`, synchronously.

use crate::SignedRequest;

impl SignedRequest {
    /// Sends the request on the given agent and returns the response. A
    /// non-2xx status is an error, as it is for every `ureq` call, unless the
    /// agent is configured otherwise.
    pub fn send_ureq(
        self,
        agent: &ureq::Agent,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        match self.method {
            "PUT" => {
                let mut request = agent.put(&self.url);
                for (name, value) in &self.headers {
                    request = request.header(name.as_str(), value.as_str());
                }
                request.send(&self.body[..])
            }
            _ => {
                let mut request = match self.method {
                    "GET" => agent.get(&self.url),
                    "HEAD" => agent.head(&self.url),
                    _ => agent.delete(&self.url),
                };
                for (name, value) in &self.headers {
                    request = request.header(name.as_str(), value.as_str());
                }
                request.call()
            }
        }
    }
}
