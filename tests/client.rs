use identity_metadata_client::{
  Error, IdentityMetadataClient, NewIssuer, OrganizationDocument, PartyId, StatusCode, Uuid,
  ValidationMethod,
};
use serde_json::{Value, json};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const IDENTIFIER: &str = "urn:oasis:names:tc:ebcore:partyid-type:iso6523:0002:552100554";
const ID: &str = "6f1c2b9e-3d4a-4e8b-9c1f-2a7d5e8b4c3a";
const TOKEN: &str = "token";

fn identifier() -> PartyId {
  IDENTIFIER.parse().unwrap()
}

fn id() -> Uuid {
  Uuid::parse_str(ID).unwrap()
}

fn document(server: &MockServer) -> Value {
  json!({
    "@context": "https://schema.org",
    "@id": format!("{}/organizations/{ID}", server.uri()),
    "@type": "Organization",
    "identifier": IDENTIFIER,
    "name": "ACME"
  })
}

fn json_ld(body: Value) -> ResponseTemplate {
  ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/ld+json")
}

fn client(server: &MockServer) -> IdentityMetadataClient {
  IdentityMetadataClient::new(&server.uri())
    .unwrap()
    .with_bearer_token(TOKEN)
}

#[tokio::test]
async fn gets_organization() {
  let server = MockServer::start().await;
  Mock::given(method("GET"))
    .and(path(format!("/organizations/{ID}")))
    .respond_with(json_ld(document(&server)))
    .expect(1)
    .mount(&server)
    .await;

  let org = IdentityMetadataClient::new(&server.uri())
    .unwrap()
    .get_organization(id())
    .await
    .unwrap();
  assert_eq!(org.name, "ACME");
  assert_eq!(org.identifier, IDENTIFIER);
  assert_eq!(org.organization_id(), Some(id()));
}

#[tokio::test]
async fn keeps_base_path() {
  let server = MockServer::start().await;
  Mock::given(method("GET"))
    .and(path(format!("/ims/organizations/{ID}")))
    .respond_with(json_ld(document(&server)))
    .expect(2)
    .mount(&server)
    .await;

  for base in ["/ims", "/ims/"] {
    let client = IdentityMetadataClient::new(&format!("{}{base}", server.uri())).unwrap();
    let _ = client.get_organization(id()).await;
  }
  server.verify().await;
  let requests = server.received_requests().await.unwrap();
  assert!(
    requests
      .iter()
      .all(|r| r.url.path() == format!("/ims/organizations/{ID}"))
  );
}

#[tokio::test]
async fn updates_organization() {
  let server = MockServer::start().await;
  let sent = json!({
    "@context": "https://schema.org",
    "@type": "Organization",
    "identifier": IDENTIFIER,
    "name": "ACME",
    "location": "Paris"
  });
  let mut returned = document(&server);
  returned["location"] = json!("Paris");
  Mock::given(method("PUT"))
    .and(path(format!("/organizations/{ID}")))
    .and(header("authorization", "Bearer token"))
    .and(header("content-type", "application/ld+json"))
    .and(body_json(&sent))
    .respond_with(json_ld(returned))
    .expect(1)
    .mount(&server)
    .await;

  let org = client(&server)
    .update_organization(
      id(),
      &OrganizationDocument::new(identifier(), "ACME").with_location("Paris"),
    )
    .await
    .unwrap();
  assert_eq!(org.location, Some("Paris".into()));
}

#[tokio::test]
async fn reports_problem_details() {
  let server = MockServer::start().await;
  Mock::given(method("GET"))
    .and(path(format!("/organizations/{ID}")))
    .respond_with(
      ResponseTemplate::new(404).set_body_raw(
        json!({
          "type": "about:blank",
          "title": "Not Found",
          "status": 404,
          "detail": format!("organization `{ID}` not found")
        })
        .to_string(),
        "application/problem+json",
      ),
    )
    .mount(&server)
    .await;

  let error = client(&server).get_organization(id()).await.unwrap_err();
  assert_eq!(error.status(), Some(StatusCode::NOT_FOUND));
  let Error::Api {
    problem: Some(problem),
    ..
  } = &error
  else {
    panic!("unexpected error: {error:?}");
  };
  assert_eq!(problem.title, "Not Found");
  assert_eq!(
    error.to_string(),
    format!("404 Not Found: organization `{ID}` not found")
  );
}

#[tokio::test]
async fn administers_organizations() {
  let server = MockServer::start().await;
  let mut item = document(&server);
  item.as_object_mut().unwrap().remove("@context");
  Mock::given(method("GET"))
    .and(path("/admin/organizations"))
    .and(header("authorization", "Bearer token"))
    .respond_with(json_ld(json!({
      "@context": "https://schema.org",
      "@graph": [item]
    })))
    .expect(1)
    .mount(&server)
    .await;
  Mock::given(method("POST"))
    .and(path("/admin/organizations"))
    .and(header("content-type", "application/ld+json"))
    .and(body_json(json!({
      "@context": "https://schema.org",
      "@type": "Organization",
      "identifier": IDENTIFIER,
      "name": "ACME"
    })))
    .respond_with(
      ResponseTemplate::new(201)
        .insert_header("location", format!("{}/organizations/{ID}", server.uri()))
        .set_body_raw(document(&server).to_string(), "application/ld+json"),
    )
    .expect(1)
    .mount(&server)
    .await;
  Mock::given(method("DELETE"))
    .and(path(format!("/admin/organizations/{ID}")))
    .respond_with(ResponseTemplate::new(204))
    .expect(1)
    .mount(&server)
    .await;

  let client = client(&server);
  let organizations = client.admin_list_organizations().await.unwrap();
  assert_eq!(organizations.len(), 1);
  assert_eq!(organizations[0].organization_id(), Some(id()));

  let created = client
    .admin_create_organization(&OrganizationDocument::new(identifier(), "ACME"))
    .await
    .unwrap();
  assert_eq!(created.organization_id(), Some(id()));

  client.admin_delete_organization(id()).await.unwrap();
}

#[tokio::test]
async fn administers_issuers() {
  let server = MockServer::start().await;
  let issuer = json!({
    "id": "0b6c4f4e-8f0a-4c43-9d8e-2f4a3b1c5d6e",
    "issuer": "https://login.example.com/realms/acme",
    "tokenValidation": "introspection",
    "audience": "ims",
    "clientId": "ims"
  });
  Mock::given(method("GET"))
    .and(path(format!("/admin/organizations/{ID}/issuers")))
    .respond_with(ResponseTemplate::new(200).set_body_json(json!([issuer])))
    .expect(1)
    .mount(&server)
    .await;
  Mock::given(method("POST"))
    .and(path(format!("/admin/organizations/{ID}/issuers")))
    .and(header("content-type", "application/json"))
    .and(body_json(json!({
      "issuer": "https://login.example.com/realms/acme",
      "audience": "ims",
      "clientId": "ims",
      "clientSecret": "s3cret"
    })))
    .respond_with(ResponseTemplate::new(201).set_body_json(&issuer))
    .expect(1)
    .mount(&server)
    .await;
  // The issuer id is a path segment: it is percent-encoded.
  Mock::given(method("DELETE"))
    .and(path(format!("/admin/organizations/{ID}/issuers/a%2Fb")))
    .respond_with(ResponseTemplate::new(204))
    .expect(1)
    .mount(&server)
    .await;

  let client = client(&server);
  let issuers = client.admin_list_issuers(id()).await.unwrap();
  assert_eq!(issuers.len(), 1);
  assert_eq!(issuers[0].token_validation, ValidationMethod::Introspection);

  let registered = client
    .admin_register_issuer(
      id(),
      &NewIssuer::introspection("https://login.example.com/realms/acme", "ims", "s3cret")
        .with_audience("ims"),
    )
    .await
    .unwrap();
  assert_eq!(registered, issuers[0]);

  client.admin_unregister_issuer(id(), "a/b").await.unwrap();
}

#[test]
fn rejects_invalid_base_url() {
  assert!(matches!(
    IdentityMetadataClient::new("not a url"),
    Err(Error::BaseUrl(_))
  ));
  assert!(matches!(
    IdentityMetadataClient::new("mailto:admin@example.com"),
    Err(Error::BaseUrl(_))
  ));
}
