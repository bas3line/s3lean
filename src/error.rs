/// What can go wrong before a request is sent. Everything after that is the
/// HTTP client's error type, which this crate does not wrap.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The endpoint is not `http://host[:port]` or `https://host[:port]`.
    Endpoint(String),
    /// The bucket name is empty.
    Bucket(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Endpoint(endpoint) => {
                write!(f, "endpoint {endpoint:?} is not scheme://host[:port]")
            }
            Error::Bucket(bucket) => write!(f, "bucket name {bucket:?} is empty"),
        }
    }
}

impl std::error::Error for Error {}
