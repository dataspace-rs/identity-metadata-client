use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::{Method, RequestBuilder, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;
use uuid::Uuid;

use identity_metadata_commons::{
  NewIssuer, OrganizationDocument, OrganizationGraph, ProblemDetails, TrustedIssuer,
};

use crate::error::{Error, Result};

const JSON_LD: &str = "application/ld+json";
const JSON: &str = "application/json";

/// Client of the identity metadata service.
///
/// The service does not issue tokens: they are obtained from an identity provider and given with
/// [`IdentityMetadataClient::with_bearer_token`]. Reading the identity metadata of an organization is public;
/// updating it requires a token of one of its trusted issuers or of an administrator, and the
/// `admin_*` operations a token of an administrator of the root identity.
#[derive(Debug, Clone)]
pub struct IdentityMetadataClient {
  http: reqwest::Client,
  base_url: Url,
  token: Option<String>,
}

impl IdentityMetadataClient {
  /// A client of the service at `base_url` (e.g. `https://metadata.example.com`).
  pub fn new(base_url: &str) -> Result<Self> {
    Self::with_http_client(reqwest::Client::new(), base_url)
  }

  /// A client sending its requests with `http`.
  pub fn with_http_client(http: reqwest::Client, base_url: &str) -> Result<Self> {
    let base_url = Url::parse(base_url).map_err(|e| Error::BaseUrl(e.to_string()))?;
    if base_url.cannot_be_a_base() {
      return Err(Error::BaseUrl(format!("`{base_url}` cannot be a base")));
    }
    Ok(Self {
      http,
      base_url,
      token: None,
    })
  }

  /// The same client, authenticated with an OAuth 2.0 access token.
  pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
    self.token = Some(token.into());
    self
  }

  pub fn base_url(&self) -> &Url {
    &self.base_url
  }

  // ---- organizations ----

  /// Gets the identity metadata of an organization.
  pub async fn get_organization(&self, id: Uuid) -> Result<OrganizationDocument> {
    let request = self.request(Method::GET, &["organizations", &id.to_string()]);
    json(send(request.header(ACCEPT, JSON_LD)).await?).await
  }

  /// Replaces the identity metadata of an organization, with a token of one of its trusted
  /// issuers or of an administrator.
  pub async fn update_organization(
    &self,
    id: Uuid,
    organization: &OrganizationDocument,
  ) -> Result<OrganizationDocument> {
    let request = self.request(Method::PUT, &["organizations", &id.to_string()]);
    json(send(json_ld_body(request, organization)?).await?).await
  }

  // ---- administration ----

  /// Lists the registered organizations.
  pub async fn admin_list_organizations(&self) -> Result<Vec<OrganizationDocument>> {
    let request = self.request(Method::GET, &["admin", "organizations"]);
    let graph: OrganizationGraph = json(send(request.header(ACCEPT, JSON_LD)).await?).await?;
    Ok(graph.graph)
  }

  /// Registers an organization with its initial identity metadata. The returned metadata holds
  /// the `@id` assigned by the service (see [`OrganizationDocument::organization_id`]).
  pub async fn admin_create_organization(
    &self,
    organization: &OrganizationDocument,
  ) -> Result<OrganizationDocument> {
    let request = self.request(Method::POST, &["admin", "organizations"]);
    json(send(json_ld_body(request, organization)?).await?).await
  }

  /// Deletes an organization with its trusted issuers.
  pub async fn admin_delete_organization(&self, id: Uuid) -> Result<()> {
    let request = self.request(Method::DELETE, &["admin", "organizations", &id.to_string()]);
    send(request).await.map(drop)
  }

  /// Lists the trusted issuers of an organization.
  pub async fn admin_list_issuers(&self, id: Uuid) -> Result<Vec<TrustedIssuer>> {
    let id = id.to_string();
    let request = self.request(Method::GET, &["admin", "organizations", &id, "issuers"]);
    json(send(request.header(ACCEPT, JSON)).await?).await
  }

  /// Trusts an identity provider for an organization: its tokens may update the organization.
  pub async fn admin_register_issuer(&self, id: Uuid, issuer: &NewIssuer) -> Result<TrustedIssuer> {
    let id = id.to_string();
    let request = self.request(Method::POST, &["admin", "organizations", &id, "issuers"]);
    let request = request
      .header(ACCEPT, JSON)
      .header(CONTENT_TYPE, JSON)
      .body(serde_json::to_vec(issuer)?);
    json(send(request).await?).await
  }

  /// Stops trusting an identity provider for an organization.
  pub async fn admin_unregister_issuer(&self, id: Uuid, issuer_id: &str) -> Result<()> {
    let id = id.to_string();
    let request = self.request(
      Method::DELETE,
      &["admin", "organizations", &id, "issuers", issuer_id],
    );
    send(request).await.map(drop)
  }

  // ---- helpers ----

  /// A request to the URL made of the base URL followed by the (percent-encoded) `segments`.
  fn request(&self, method: Method, segments: &[&str]) -> RequestBuilder {
    let mut url = self.base_url.clone();
    url
      .path_segments_mut()
      .expect("base URL checked at construction")
      .pop_if_empty()
      .extend(segments);
    let request = self.http.request(method, url);
    match &self.token {
      Some(token) => request.bearer_auth(token),
      None => request,
    }
  }
}

fn json_ld_body<T: Serialize>(request: RequestBuilder, body: &T) -> Result<RequestBuilder> {
  Ok(
    request
      .header(ACCEPT, JSON_LD)
      .header(CONTENT_TYPE, JSON_LD)
      .body(serde_json::to_vec(body)?),
  )
}

/// Sends a request, turning error statuses into [`Error::Api`].
async fn send(request: RequestBuilder) -> Result<Response> {
  let response = request.send().await?;
  let status = response.status();
  if status.is_success() {
    return Ok(response);
  }
  let body = response.text().await?;
  let problem = serde_json::from_str::<ProblemDetails>(&body).ok();
  Err(Error::Api {
    status,
    problem,
    body,
  })
}

async fn json<T: DeserializeOwned>(response: Response) -> Result<T> {
  Ok(serde_json::from_slice(&response.bytes().await?)?)
}
