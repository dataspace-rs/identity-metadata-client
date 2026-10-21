//! Client of the [identity metadata service](https://github.com/dataspace-rs/identity-metadata-service),
//! publishing the public identity metadata of organizations as JSON-LD (schema.org
//! `Organization`).
//!
//! ```no_run
//! # async fn run() -> identity_metadata_client::Result<()> {
//! use identity_metadata_client::{IdentityMetadataClient, NewIssuer, OrganizationDocument, PartyId};
//!
//! let admin = IdentityMetadataClient::new("https://metadata.example.com")?.with_bearer_token("<admin token>");
//! let acme = admin
//!   .admin_create_organization(&OrganizationDocument::new(
//!     "urn:oasis:names:tc:ebcore:partyid-type:iso6523:0002:552100554"
//!       .parse::<PartyId>()
//!       .expect("valid Party Id"),
//!     "ACME",
//!   ))
//!   .await?;
//! let id = acme.organization_id().expect("assigned by the service");
//! admin
//!   .admin_register_issuer(
//!     id,
//!     &NewIssuer::jwks("https://login.example.com/realms/acme")
//!       .with_audience("identity-metadata-service"),
//!   )
//!   .await?;
//!
//! let public = IdentityMetadataClient::new("https://metadata.example.com")?;
//! assert_eq!(public.get_organization(id).await?.name, "ACME");
//! # Ok(())
//! # }
//! ```

mod error;
mod identity_metadata_client;

pub use error::{Error, Result};
pub use identity_metadata_client::IdentityMetadataClient;
pub use identity_metadata_commons::{
  Address, Location, NewIssuer, OrganizationDocument, PartyId, PartyIdError, Place, PostalAddress,
  ProblemDetails, TrustedIssuer, ValidationMethod,
};
pub use reqwest::StatusCode;
pub use uuid::Uuid;
