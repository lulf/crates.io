use crate::util::{RequestHelper, TestApp};
use cargo_registry::models::ApiToken;
use cargo_registry::views::EncodableApiTokenWithToken;
use diesel::prelude::*;
use http::StatusCode;

static NEW_BAR: &[u8] = br#"{ "api_token": { "name": "bar" } }"#;

#[derive(Deserialize)]
struct NewResponse {
    api_token: EncodableApiTokenWithToken,
}

#[test]
fn create_token_logged_out() {
    let (_, anon) = TestApp::init().empty();
    anon.put("/api/v1/me/tokens", NEW_BAR).assert_forbidden();
}

#[test]
fn create_token_invalid_request() {
    let (_, _, user) = TestApp::init().with_user();
    let invalid = br#"{ "name": "" }"#;
    let response = user.put::<()>("/api/v1/me/tokens", invalid);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.into_json(),
        json!({ "errors": [{ "detail": "invalid new token request: Error(\"missing field `api_token`\", line: 1, column: 14)" }] })
    );
}

#[test]
fn create_token_no_name() {
    let (_, _, user) = TestApp::init().with_user();
    let empty_name = br#"{ "api_token": { "name": "" } }"#;
    let response = user.put::<()>("/api/v1/me/tokens", empty_name);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.into_json(),
        json!({ "errors": [{ "detail": "name must have a value" }] })
    );
}

#[test]
fn create_token_long_body() {
    let (_, _, user) = TestApp::init().with_user();
    let too_big = &[5; 5192]; // Send a request with a 5kB body of 5's
    let response = user.put::<()>("/api/v1/me/tokens", too_big);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.into_json(),
        json!({ "errors": [{ "detail": "max content length is: 2000" }] })
    );
}

#[test]
fn create_token_exceeded_tokens_per_user() {
    let (app, _, user) = TestApp::init().with_user();
    let id = user.as_model().id;
    app.db(|conn| {
        for i in 0..1000 {
            assert_ok!(ApiToken::insert(conn, id, &format!("token {i}")));
        }
    });
    let response = user.put::<()>("/api/v1/me/tokens", NEW_BAR);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.into_json(),
        json!({ "errors": [{ "detail": "maximum tokens per user is: 500" }] })
    );
}

#[test]
fn create_token_success() {
    let (app, _, user) = TestApp::init().with_user();

    let json: NewResponse = user.put("/api/v1/me/tokens", NEW_BAR).good();
    assert_eq!(json.api_token.name, "bar");
    assert!(!json.api_token.token.is_empty());

    let tokens: Vec<ApiToken> =
        app.db(|conn| assert_ok!(ApiToken::belonging_to(user.as_model()).load(conn)));
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "bar");
    assert!(!tokens[0].revoked);
    assert_eq!(tokens[0].last_used_at, None);
}

#[test]
fn create_token_multiple_have_different_values() {
    let (_, _, user) = TestApp::init().with_user();
    let first: NewResponse = user.put("/api/v1/me/tokens", NEW_BAR).good();
    let second: NewResponse = user.put("/api/v1/me/tokens", NEW_BAR).good();

    assert_ne!(first.api_token.token, second.api_token.token);
}

#[test]
fn create_token_multiple_users_have_different_values() {
    let (app, _, user1) = TestApp::init().with_user();
    let first_token: NewResponse = user1.put("/api/v1/me/tokens", NEW_BAR).good();

    let user2 = app.db_new_user("bar");
    let second_token: NewResponse = user2.put("/api/v1/me/tokens", NEW_BAR).good();

    assert_ne!(first_token.api_token.token, second_token.api_token.token);
}

#[test]
fn cannot_create_token_with_token() {
    let (_, _, _, token) = TestApp::init().with_token();
    let response = token.put::<()>(
        "/api/v1/me/tokens",
        br#"{ "api_token": { "name": "baz" } }"#,
    );
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.into_json(),
        json!({ "errors": [{ "detail": "cannot use an API token to create a new API token" }] })
    );
}
