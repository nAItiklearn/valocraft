use argon2::{Config, Variant};
pub use auth_common::AuthToken;
use auth_common::{
    RegisterPayload, SignInPayload, SignInResponse, UsernameLookupPayload, UsernameLookupResponse,
    UuidLookupPayload, UuidLookupResponse, ValidityCheckPayload, ValidityCheckResponse,
};
use bytes::Bytes;
pub use http::uri::Authority;
pub use http::uri::Scheme;
use http::Request;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::Uri;
use hyper_util::{client::legacy, rt::TokioExecutor};
pub use uuid::Uuid;

fn net_prehash(password: &str) -> String {
    let salt = fxhash::hash64(password);
    let config = Config {
        variant: Variant::Argon2i,
        time_cost: 3,
        mem_cost: 4096,
        ..Default::default()
    };
    let bytes = argon2::hash_raw(password.as_bytes(), &salt.to_le_bytes(), &config).unwrap();
    hex::encode(bytes)
}

#[derive(Debug)]
pub enum AuthClientError {
    // Server did not return 200-299 StatusCode.
    ServerError(u16, Vec<u8>),
    RequestError(legacy::Error),
    ResponseError(hyper::Error),
    JsonError(serde_json::Error),
    CertificateLoad(std::io::Error),
    InsecureSchema,
}
pub struct AuthClient {
    client:
        legacy::Client<hyper_rustls::HttpsConnector<legacy::connect::HttpConnector>, Full<Bytes>>,
    //precached Parts
    register_uri: Uri,
    username_to_uuid_uri: Uri,
    uuid_to_username_uri: Uri,
    generate_token_uri: Uri,
    verify_uri: Uri,
}

impl AuthClient {
    pub fn new(scheme: Scheme, authority: Authority) -> Result<Self, AuthClientError> {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(AuthClientError::CertificateLoad)?
            .https_or_http()
            .enable_http1()
            .build();
        let client: legacy::Client<_, Full<Bytes>> =
            legacy::Client::builder(TokioExecutor::new()).build(https);

        Self::with_client(scheme, authority, client)
    }

    pub fn with_client(
        scheme: Scheme,
        authority: Authority,
        client: legacy::Client<
            hyper_rustls::HttpsConnector<legacy::connect::HttpConnector>,
            Full<Bytes>,
        >,
    ) -> Result<Self, AuthClientError> {
        // enforce HTTPS except `localhost` and/or `debug` build
        #[cfg(not(debug_assertions))]
        {
            if scheme == Scheme::HTTP && authority.host() != "localhost" {
                return Err(AuthClientError::InsecureSchema);
            }
        }

        let register_uri = Self::get_uri(&scheme, &authority, "/register");
        let username_to_uuid_uri = Self::get_uri(&scheme, &authority, "/username_to_uuid");
        let uuid_to_username_uri = Self::get_uri(&scheme, &authority, "/uuid_to_username");
        let generate_token_uri = Self::get_uri(&scheme, &authority, "/generate_token");
        let verify_uri = Self::get_uri(&scheme, &authority, "/verify");

        Ok(Self {
            client,
            register_uri,
            username_to_uuid_uri,
            uuid_to_username_uri,
            generate_token_uri,
            verify_uri,
        })
    }

    async fn post<T>(
        &self,
        uri: &Uri,
        data: T,
    ) -> std::result::Result<http::Response<hyper::body::Incoming>, AuthClientError>
    where
        T: serde::ser::Serialize,
    {
        let body = serde_json::to_vec(&data)?;
        let body: Bytes = body.into();

        let mut request = Request::new(Full::new(body));
        *request.method_mut() = hyper::Method::POST;
        *request.uri_mut() = uri.clone();
        self.client
            .request(request)
            .await
            .map_err(AuthClientError::RequestError)
    }

    pub async fn register(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<(), AuthClientError> {
        let data = RegisterPayload {
            username: username.as_ref().to_owned(),
            password: net_prehash(password.as_ref()),
        };
        self.post(&self.register_uri, data).await?;
        Ok(())
    }

    fn get_uri(scheme: &Scheme, authority: &Authority, path: &'static str) -> Uri {
        Uri::builder()
            .scheme(scheme.clone())
            .authority(authority.clone())
            .path_and_query(http::uri::PathAndQuery::from_static(path))
            .build()
            .expect("This URI should always be correct, so this will never panic")
    }

    pub async fn username_to_uuid(
        &self,
        username: impl AsRef<str>,
    ) -> Result<Uuid, AuthClientError> {
        let data = UuidLookupPayload {
            username: username.as_ref().to_owned(),
        };
        let resp = self.post(&self.username_to_uuid_uri, data).await?;

        Ok(handle_response::<UuidLookupResponse>(resp).await?.uuid)
    }

    pub async fn uuid_to_username(&self, uuid: Uuid) -> Result<String, AuthClientError> {
        let data = UsernameLookupPayload { uuid };
        let resp = self.post(&self.uuid_to_username_uri, data).await?;

        Ok(handle_response::<UsernameLookupResponse>(resp)
            .await?
            .username)
    }

    pub async fn sign_in(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<AuthToken, AuthClientError> {
        let data = SignInPayload {
            username: username.as_ref().to_owned(),
            password: net_prehash(password.as_ref()),
        };
        let resp = self.post(&self.generate_token_uri, data).await?;

        Ok(handle_response::<SignInResponse>(resp).await?.token)
    }

    pub async fn validate(&self, token: AuthToken) -> Result<Uuid, AuthClientError> {
        let data = ValidityCheckPayload { token };
        let resp = self.post(&self.verify_uri, data).await?;

        Ok(handle_response::<ValidityCheckResponse>(resp).await?.uuid)
    }
}

/// If response code isn't a success it will return an error with the response code and plain text body.
///
/// Otherwise will deserialize the json based on given type (through turbofish notation)
async fn handle_response<T>(resp: hyper::Response<Incoming>) -> Result<T, AuthClientError>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let status = resp.status();
    let body: Bytes = resp
        .collect()
        .await
        .map_err(AuthClientError::ResponseError)?
        .to_bytes();

    if status.is_success() {
        Ok(serde_json::from_slice(&body)?)
    } else {
        Err(AuthClientError::ServerError(status.as_u16(), body.to_vec()))
    }
}

impl std::fmt::Display for AuthClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            AuthClientError::ServerError(code, text) => {
                write!(f, "Auth Server returned {} with: {:?}", code, text)
            }
            AuthClientError::RequestError(text) => write!(f, "Request failed with: {}", text),
            AuthClientError::ResponseError(text) => write!(f, "Reading Response failed with: {}", text),
            AuthClientError::JsonError(text) => write!(f, "Failed to convert Json with: {}", text),
            AuthClientError::InsecureSchema => write!(f, "Using auth with `HTTP` is insecure. It's only allowed to use HTTP if the authority is `localhost` or when debug_assertions are set"),
            AuthClientError::CertificateLoad(text) => write!(f, "reading OS certificates to connect with server wasn't possible: {}", text),
        }
    }
}

impl From<serde_json::Error> for AuthClientError {
    fn from(err: serde_json::Error) -> Self {
        AuthClientError::JsonError(err)
    }
}
