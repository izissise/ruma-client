//! Crate `ruma_client` is a [Matrix](https://matrix.org/) client library.
//!
//! # Usage
//!
//! Begin by creating a `Client` type, usually using the `https` method for a client that supports
//! secure connections, and then logging in:
//!
//! ```no_run
//! use ruma_client::Client;
//!
//! let work = async {
//!     let homeserver_url = "https://example.com".parse().unwrap();
//!     let client = Client::https(homeserver_url, None);
//!
//!     let session = client
//!         .log_in("@alice:example.com".to_string(), "secret".to_string(), None)
//!         .await?;
//!
//!     // You're now logged in! Write the session to a file if you want to restore it later.
//!     // Then start using the API!
//! # Result::<(), ruma_client::Error>::Ok(())
//! };
//! ```
//!
//! You can also pass an existing session to the `Client` constructor to restore a previous session
//! rather than calling `log_in`.
//!
//! For the standard use case of synchronizing with the homeserver (i.e. getting all the latest
//! events), use the `Client::sync`:
//!
//! ```no_run
//! # use futures_util::stream::{StreamExt as _, TryStreamExt as _};
//! # use ruma_client::Client;
//! # let homeserver_url = "https://example.com".parse().unwrap();
//! # let client = Client::https(homeserver_url, None);
//! # async {
//! let mut sync_stream = Box::pin(client.sync(None, None, true));
//! while let Some(response) = sync_stream.try_next().await? {
//!     // Do something with the data in the response...
//! }
//! # Result::<(), ruma_client::Error>::Ok(())
//! # };
//! ```
//!
//! The `Client` type also provides methods for registering a new account if you don't already have
//! one with the given homeserver.
//!
//! Beyond these basic convenience methods, `ruma-client` gives you access to the entire Matrix
//! client-server API via the `api` module. Each leaf module under this tree of modules contains
//! the necessary types for one API endpoint. Simply call the module's `call` method, passing it
//! the logged in `Client` and the relevant `Request` type. `call` will return a future that will
//! resolve to the relevant `Response` type.
//!
//! For example:
//!
//! ```no_run
//! # use ruma_client::Client;
//! # let homeserver_url = "https://example.com".parse().unwrap();
//! # let client = Client::https(homeserver_url, None);
//! use std::convert::TryFrom;
//!
//! use ruma_client::api::r0::alias::get_alias;
//! use ruma_identifiers::{RoomAliasId, RoomId};
//!
//! async {
//!     let response = client
//!         .request(get_alias::Request {
//!             room_alias: RoomAliasId::try_from("#example_room:example.com").unwrap(),
//!         })
//!         .await?;
//!
//!     assert_eq!(response.room_id, RoomId::try_from("!n8f893n9:example.com").unwrap());
//! #   Result::<(), ruma_client::Error>::Ok(())
//! }
//! # ;
//! ```

#![warn(rust_2018_idioms)]
#![deny(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs
)]

use std::{
    convert::TryFrom,
    str::FromStr,
    sync::{Arc, Mutex},
};

use futures_core::{
    future::Future,
    stream::{Stream, TryStream},
};
use tower_service::Service;
use futures_util::stream;
use http::{Request as HttpRequest, Response as HttpResponse, Uri};
use ruma_api::{Endpoint, Outgoing};
use url::Url;

use crate::error::InnerError;

#[cfg(feature = "hyper_client")]
pub use hyper_client::HttpClient;
#[cfg(feature = "tls")]
pub use hyper_client::HttpsClient;


pub use crate::{
    error::Error,
    session::Session,
};
pub use ruma_client_api as api;
pub use ruma_events as events;
pub use ruma_identifiers as identifiers;

/// Matrix client-server API endpoints.
//pub mod api;
mod error;
mod session;

/// A client for the Matrix client-server API.
#[derive(Debug)]
pub struct Client<C: Service<HttpRequest<Vec<u8>>>>(Arc<ClientData<C>>);

/// Data contained in Client's Rc
#[derive(Debug)]
struct ClientData<C>
where
    C: Service<HttpRequest<Vec<u8>>>,
{
    /// The URL of the homeserver to connect to.
    homeserver_url: Url,
    /// The underlying HTTP client.
    http_client: C,
    /// User session data.
    session: Mutex<Option<Session>>,
}

trait TypeEquals {
    type Other;
}

impl<T, REQ> TypeEquals for <T as Service<REQ>>::Response {
    type Other = Self;
}

impl TypeEquals for HttpResponse<Vec<u8>> {
    type Other = Self;
}

impl<C> Client<C>
where
    C: Service<HttpRequest<Vec<u8>>> + 'static,
    Error: std::convert::From<<C as Service<HttpRequest<Vec<u8>>>>::Error>,
    <C as Service<HttpRequest<Vec<u8>>>>::Response: TypeEquals<Other= HttpResponse<Vec<u8>>>,
//     HttpResponse<Vec<u8>>: TypeEquals<Other=<C as Service<HttpRequest<Vec<u8>>>>::Response>,
{
    /// Creates a new client using the given `HttpRequester`.
    ///
    /// This allows the user to configure the details of HTTP as desired.
    /// And to pass a session if one already exist with the homeserver
    pub fn new(http_client: C, homeserver_url: Url, session: Option<Session>) -> Self {
        Self(Arc::new(ClientData {
            homeserver_url,
            http_client: http_client,
            session: Mutex::new(session),
        }))
    }

    /// Get a copy of the current `Session`, if any.
    ///
    /// Useful for serializing and persisting the session to be restored later.
    pub fn session(&self) -> Option<Session> {
        self.0
            .session
            .lock()
            .expect("session mutex was poisoned")
            .clone()
    }

    /// Log in with a username and password.
    ///
    /// In contrast to `api::r0::session::login::call()`, this method stores the
    /// session data returned by the endpoint in this client, instead of
    /// returning it.
    pub async fn log_in(
        &self,
        user: String,
        password: String,
        device_id: Option<String>,
    ) -> Result<Session, Error> {
        use api::r0::session::login;

        let response = self
            .request(login::Request {
                address: None,
                login_type: login::LoginType::Password,
                medium: None,
                device_id,
                password,
                user,
            })
            .await?;

        let session = Session {
            access_token: response.access_token,
            device_id: response.device_id,
            user_id: response.user_id,
        };
        *self.0.session.lock().unwrap() = Some(session.clone());

        Ok(session)
    }

    /// Register as a guest. In contrast to `api::r0::account::register::call()`,
    /// this method stores the session data returned by the endpoint in this
    /// client, instead of returning it.
    pub async fn register_guest(&self) -> Result<Session, Error> {
        use api::r0::account::register;

        let response = self
            .request(register::Request {
                auth: None,
                bind_email: None,
                device_id: None,
                initial_device_display_name: None,
                kind: Some(register::RegistrationKind::Guest),
                password: None,
                username: None,
            })
            .await?;

        let session = Session {
            access_token: response.access_token,
            device_id: response.device_id,
            user_id: response.user_id,
        };
        *self.0.session.lock().unwrap() = Some(session.clone());

        Ok(session)
    }

    /// Register as a new user on this server.
    ///
    /// In contrast to `api::r0::account::register::call()`, this method stores
    /// the session data returned by the endpoint in this client, instead of
    /// returning it.
    ///
    /// The username is the local part of the returned user_id. If it is
    /// omitted from this request, the server will generate one.
    pub async fn register_user(
        &self,
        username: Option<String>,
        password: String,
    ) -> Result<Session, Error> {
        use api::r0::account::register;

        let response = self
            .request(register::Request {
                auth: None,
                bind_email: None,
                device_id: None,
                initial_device_display_name: None,
                kind: Some(register::RegistrationKind::User),
                password: Some(password),
                username,
            })
            .await?;

        let session = Session {
            access_token: response.access_token,
            device_id: response.device_id,
            user_id: response.user_id,
        };
        *self.0.session.lock().unwrap() = Some(session.clone());

        Ok(session)
    }

    /// Convenience method that represents repeated calls to the sync_events endpoint as a stream.
    ///
    /// If the since parameter is None, the first Item might take a significant time to arrive and
    /// be deserialized, because it contains all events that have occurred in the whole lifetime of
    /// the logged-in users account and are visible to them.
    pub fn sync(
        &self,
        filter: Option<api::r0::sync::sync_events::Filter>,
        since: Option<String>,
        set_presence: bool,
    ) -> impl Stream<Item = Result<api::r0::sync::sync_events::IncomingResponse, Error>>
           + TryStream<Ok = api::r0::sync::sync_events::IncomingResponse, Error = Error> {
        use api::r0::sync::sync_events;

        // TODO: Is this really the way TryStreams are supposed to work?
        #[derive(Debug, PartialEq, Eq)]
        enum State {
            InitialSync,
            Since(String),
            Errored,
        }

        let client = self.clone();
        let set_presence = if set_presence {
            None
        } else {
            Some(sync_events::SetPresence::Offline)
        };

        let initial_state = match since {
            Some(s) => State::Since(s),
            None => State::InitialSync,
        };

        stream::unfold(initial_state, move |state| {
            let client = client.clone();
            let filter = filter.clone();

            async move {
                let since = match state {
                    State::Errored => return None,
                    State::Since(s) => Some(s),
                    State::InitialSync => None,
                };

                let res = client
                    .request(sync_events::Request {
                        filter,
                        since,
                        full_state: None,
                        set_presence,
                        timeout: None,
                    })
                    .await;

                match res {
                    Ok(response) => {
                        let next_batch_clone = response.next_batch.clone();
                        Some((Ok(response), State::Since(next_batch_clone)))
                    }
                    Err(e) => Some((Err(e), State::Errored)),
                }
            }
        })
    }

    /// Makes a request to a Matrix API endpoint.
    pub fn request<Request: Endpoint>(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<<Request::Response as Outgoing>::Incoming, Error>>
    // We need to duplicate Endpoint's where clauses because the compiler is not smart enough yet.
    // See https://github.com/rust-lang/rust/issues/54149
    where
        Request::Incoming: TryFrom<http::Request<Vec<u8>>, Error = ruma_api::Error>,
        <Request::Response as Outgoing>::Incoming:
            TryFrom<http::Response<Vec<u8>>, Error = ruma_api::Error>,
        <C as Service<HttpRequest<Vec<u8>>>>::Response: TypeEquals<Other= HttpResponse<Vec<u8>>>,

    {
        let client = self.0.clone();

        let mut url = client.homeserver_url.clone();

        async move {
            // Convert matrix request to plain http request type
            // Retrieving the url endpoint from the metadata
            let mut request = request.try_into()?;
            let uri = request.uri();
            url.set_path(uri.path());
            url.set_query(uri.query());
            if Request::METADATA.requires_authentication {
                if let Some(ref session) = *client.session.lock().unwrap() {
                    url.query_pairs_mut()
                        .append_pair("access_token", &session.access_token);
                } else {
                    return Err(Error(InnerError::AuthenticationRequired));
                }
            }
            *request.uri_mut() = Uri::from_str(url.as_ref())?;

            // Do the actual async request
            let response = client.http_client.call(request).await?;
            let ruma_rep = <Request::Response as Outgoing>::Incoming::try_from(response)?;
            Ok(ruma_rep)
        }
    }
}

impl<C: Service<HttpRequest<Vec<u8>>>> Clone for Client<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
