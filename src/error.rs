use identity_metadata_commons::ProblemDetails;
use reqwest::StatusCode;

/// Errors of the client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The request could not be sent, or its response could not be read.
  #[error(transparent)]
  Http(#[from] reqwest::Error),
  /// The base URL of the service is invalid.
  #[error("invalid base URL: {0}")]
  BaseUrl(String),
  /// The service answered with an error status.
  #[error("{status}: {}", problem.as_ref().map_or(body.as_str(), |p| p.detail.as_str()))]
  Api {
    status: StatusCode,
    /// The problem details of the response, when its body is one.
    problem: Option<ProblemDetails>,
    /// The raw body of the response.
    body: String,
  },
  /// The body of a successful response could not be decoded.
  #[error("invalid response: {0}")]
  Decode(#[from] serde_json::Error),
}

impl Error {
  /// The status of an error response of the service.
  pub fn status(&self) -> Option<StatusCode> {
    match self {
      Self::Api { status, .. } => Some(*status),
      Self::Http(e) => e.status(),
      _ => None,
    }
  }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
