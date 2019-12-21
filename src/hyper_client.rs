pub use crate::{
    error::{Error, HttpRequesterError},
};

use hyper::error::Error as HyperError;
use http::Response as HttpResponse;
use hyper::{client::HttpConnector, Client as HyperClient, Uri};
#[cfg(feature = "hyper-tls")]
use hyper_tls::HttpsConnector;
use std::pin::Pin;


impl From<HyperError> for Error {
    fn from(_error: HyperError) -> Self {
        Self(InnerError::HttpRequesterError)

/// Non-secured variant of the client (using plain HTTP requests)
pub type HttpClient = Client<HttpConnector>;

impl HttpClient {
    /// Creates a new client for making HTTP requests to the given homeserver.
    pub fn new(homeserver_url: Url, session: Option<Session>) -> Self {
        Self(Arc::new(ClientData {
            homeserver_url,
            http_client: HyperClient::builder().keep_alive(true).build_http(),
            session: Mutex::new(session),
        }))
    }
}

/// Secured variant of the client (using HTTPS requests)
#[cfg(feature = "tls")]
pub type HttpsClient = Client<HttpsConnector<HttpConnector>>;

#[cfg(feature = "tls")]
impl HttpsClient {
    /// Creates a new client for making HTTPS requests to the given homeserver.
    pub fn https(homeserver_url: Url, session: Option<Session>) -> Self {
        let connector = HttpsConnector::new();

        Self(Arc::new(ClientData {
            homeserver_url,
            http_client: HyperClient::builder().keep_alive(true).build(connector),
            session: Mutex::new(session),
        }))
    }
}
