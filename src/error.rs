//! Error conditions.

use std::error::Error as StdError;
use std::fmt::{Display, Formatter, Result as FmtResult};

use http::uri::InvalidUri;
use ruma_api::Error as RumaApiError;
use serde_json::Error as SerdeJsonError;
use serde_urlencoded::ser::Error as SerdeUrlEncodedSerializeError;

/// An error that can occur during client operations.
#[derive(Debug)]
pub struct Error(pub(crate) InnerError);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let message = match self.0 {
            InnerError::AuthenticationRequired => "The queried endpoint requires authentication but was called with an anonymous client.",
            InnerError::HttpRequester => "An HTTP error occurred.",
            InnerError::Uri(_) => "Provided string could not be converted into a URI.",
            InnerError::RumaApi(_) => "An error occurred converting between ruma_client_api and hyper types.",
            InnerError::SerdeJson(_) => "A serialization error occurred.",
            InnerError::SerdeUrlEncodedSerialize(_) => "An error occurred serializing data to a query string.",
        };

        write!(f, "{}", message)
    }
}

impl StdError for Error {}

/// An error that can occur in the HttpRequester.
#[derive(Debug, Copy)]
pub struct HttpRequesterError;
impl Display for HttpRequesterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", "HttpRequesterError")
    }
}

impl StdError for HttpRequesterError {}
impl Clone for HttpRequesterError {
    fn clone(&self) -> Self {
        Self {}
    }
}

/// Internal representation of errors.
#[derive(Debug)]
pub(crate) enum InnerError {
    /// Queried endpoint requires authentication but was called on an anonymous client.
    AuthenticationRequired,
    /// An error at the HTTP layer.
    HttpRequester,
    /// An error when parsing a string as a URI.
    Uri(InvalidUri),
    /// An error converting between ruma_client_api types and Hyper types.
    RumaApi(RumaApiError),
    /// An error when serializing or deserializing a JSON value.
    SerdeJson(SerdeJsonError),
    /// An error when serializing a query string value.
    SerdeUrlEncodedSerialize(SerdeUrlEncodedSerializeError),
}

impl From<HttpRequesterError> for Error {
    fn from(_error: HttpRequesterError) -> Self {
        Self(InnerError::HttpRequester)
    }
}

impl From<InvalidUri> for Error {
    fn from(error: InvalidUri) -> Self {
        Self(InnerError::Uri(error))
    }
}

impl From<RumaApiError> for Error {
    fn from(error: RumaApiError) -> Self {
        Self(InnerError::RumaApi(error))
    }
}

impl From<SerdeJsonError> for Error {
    fn from(error: SerdeJsonError) -> Self {
        Self(InnerError::SerdeJson(error))
    }
}

impl From<SerdeUrlEncodedSerializeError> for Error {
    fn from(error: SerdeUrlEncodedSerializeError) -> Self {
        Self(InnerError::SerdeUrlEncodedSerialize(error))
    }
}
