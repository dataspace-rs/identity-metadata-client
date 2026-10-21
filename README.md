# Identity Metadata Client

Async Rust client (reqwest) of the
[Identity Metadata Service](https://github.com/dataspace-rs/identity-metadata-service), which
publishes the public identity metadata of organizations as JSON-LD
([schema.org `Organization`](https://schema.org/Organization)).

## Usage

```rust
use identity_metadata_client::{
  IdentityMetadataClient, NewIssuer, OrganizationDocument, PartyId, Place, PostalAddress,
};

let admin = IdentityMetadataClient::new("https://metadata.example.com")?.with_bearer_token("<admin token>");

// Register an organization: the service assigns its id.
let identifier: PartyId = "urn:oasis:names:tc:ebcore:partyid-type:iso6523:0002:552100554"
  .parse()
  .expect("valid Party Id");
let acme = admin
  .admin_create_organization(
    &OrganizationDocument::new(identifier, "ACME")
      .with_legal_name("ACME SAS")
      .with_location(Place {
        name: Some("Head office".into()),
        address: Some(
          PostalAddress {
            address_locality: Some("Paris".into()),
            address_country: Some("FR".into()),
            ..Default::default()
          }
          .into(),
        ),
        ..Default::default()
      }),
  )
  .await?;
let id = acme.organization_id().expect("assigned by the service");

// Trust its identity provider, whose tokens may then update the organization.
admin
  .admin_register_issuer(
    id,
    &NewIssuer::jwks("https://login.example.com/realms/acme").with_audience("identity-metadata-service"),
  )
  .await?;

// Public read.
let org = IdentityMetadataClient::new("https://metadata.example.com")?.get_organization(id).await?;
```

## API coverage

| Method | Path | Client |
|---|---|---|
| `GET` | `/organizations/{id}` | `get_organization` |
| `PUT` | `/organizations/{id}` | `update_organization` |
| `GET` | `/admin/organizations` | `admin_list_organizations` |
| `POST` | `/admin/organizations` | `admin_create_organization` |
| `DELETE` | `/admin/organizations/{id}` | `admin_delete_organization` |
| `GET` | `/admin/organizations/{id}/issuers` | `admin_list_issuers` |
| `POST` | `/admin/organizations/{id}/issuers` | `admin_register_issuer` |
| `DELETE` | `/admin/organizations/{id}/issuers/{issuerId}` | `admin_unregister_issuer` |

The `/health`, `/openapi.json` and `/swagger-ui` endpoints are not covered.

The client does not obtain tokens: get an OAuth 2.0 access token from the identity provider and
pass it with `IdentityMetadataClient::with_bearer_token`. Errors of the service (`application/problem+json`) are
returned as `Error::Api`, with the status and the problem details.

The model shared with the service (organization documents, Party Ids, trusted issuers, problem
details) comes from the [`identity-metadata-commons`](https://crates.io/crates/identity-metadata-commons)
crate; the client re-exports its types.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
