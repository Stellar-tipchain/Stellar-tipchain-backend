use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{
    config::Config,
    routes,
    services::stellar::StellarService,
    state::AppState,
};

/// Spawn a test application with a fresh database and a mock Stellar/Horizon server.
async fn spawn_app(pool: PgPool) -> (TestServer, MockServer) {
    // Start a mock Horizon server.
    let mock_server = MockServer::start().await;

    // Build the application state pointing Stellar at the mock server.
    let stellar = StellarService::with_base_url(mock_server.uri());

    let config = Config {
        database_url: String::new(),
        host: "127.0.0.1".to_string(),
        port: 0,
        stellar_horizon_url: mock_server.uri(),
        stellar_network_passphrase: "Test SDF Network ; September 2015".to_string(),
    };

    let state = AppState {
        pool,
        stellar,
        config,
    };

    let app = routes::create_router(state);
    let server = TestServer::new(app).expect("failed to build test server");

    (server, mock_server)
}

/// Helper to build a valid-looking Stellar transaction hash (64 hex chars).
fn valid_tx_hash() -> String {
    "a".repeat(64)
}

/// Helper to build an invalid transaction hash.
fn invalid_tx_hash() -> String {
    "not-a-valid-hash".to_string()
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn health_check_works(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let response = server.get("/health").await;

    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["status"], "ok");
}

// ---------------------------------------------------------------------------
// Creators
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn create_creator_success(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });

    let response = server.post("/creators").json(&payload).await;

    response.assert_status(StatusCode::CREATED);
    let body: Value = response.json();
    assert_eq!(body["username"], "alice");
    assert_eq!(
        body["wallet_address"],
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    );
    assert!(body["id"].is_string());
}

#[sqlx::test]
async fn create_creator_duplicate_username(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });

    let first = server.post("/creators").json(&payload).await;
    first.assert_status(StatusCode::CREATED);

    let second = server.post("/creators").json(&payload).await;
    second.assert_status(StatusCode::CONFLICT);
}

#[sqlx::test]
async fn create_creator_invalid_username(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let payload = json!({
        "username": "",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });

    let response = server.post("/creators").json(&payload).await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn create_creator_invalid_wallet(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let payload = json!({
        "username": "bob",
        "wallet_address": "not-a-valid-wallet"
    });

    let response = server.post("/creators").json(&payload).await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn get_creator_success(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });

    let created = server.post("/creators").json(&payload).await;
    created.assert_status(StatusCode::CREATED);
    let created_body: Value = created.json();
    let id = created_body["id"].as_str().unwrap().to_string();

    let response = server.get(&format!("/creators/{}", id)).await;

    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["username"], "alice");
    assert_eq!(body["id"], id);
}

#[sqlx::test]
async fn get_creator_not_found(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let missing = Uuid::new_v4();
    let response = server.get(&format!("/creators/{}", missing)).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tips
// ---------------------------------------------------------------------------

/// Mount a successful Horizon transaction response for the given hash.
async fn mount_horizon_success(mock_server: &MockServer, tx_hash: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/transactions/{}", tx_hash)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hash": tx_hash,
            "successful": true,
            "ledger": 12345,
            "created_at": "2024-01-01T00:00:00Z"
        })))
        .mount(mock_server)
        .await;
}

#[sqlx::test]
async fn create_tip_success(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    // Create a creator first.
    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();
    mount_horizon_success(&mock_server, &tx_hash).await;

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash,
        "message": "great work"
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::CREATED);
    let body: Value = response.json();
    assert_eq!(body["creator_id"], creator_id);
    assert_eq!(body["transaction_hash"], tx_hash);
    assert_eq!(body["message"], "great work");
}

#[sqlx::test]
async fn create_tip_horizon_404(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();

    Mock::given(method("GET"))
        .and(path(format!("/transactions/{}", tx_hash)))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn create_tip_horizon_unsuccessful(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();

    Mock::given(method("GET"))
        .and(path(format!("/transactions/{}", tx_hash)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hash": tx_hash,
            "successful": false,
            "ledger": 12345,
            "created_at": "2024-01-01T00:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn create_tip_horizon_down(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();

    Mock::given(method("GET"))
        .and(path(format!("/transactions/{}", tx_hash)))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::BAD_GATEWAY);
}

#[sqlx::test]
async fn create_tip_duplicate_transaction_hash(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();
    mount_horizon_success(&mock_server, &tx_hash).await;

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash
    });

    let first = server.post("/tips").json(&payload).await;
    first.assert_status(StatusCode::CREATED);

    let second = server.post("/tips").json(&payload).await;
    second.assert_status(StatusCode::CONFLICT);
}

#[sqlx::test]
async fn create_tip_unknown_creator(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let tx_hash = valid_tx_hash();
    mount_horizon_success(&mock_server, &tx_hash).await;

    let payload = json!({
        "creator_id": Uuid::new_v4().to_string(),
        "amount": "10.0000000",
        "transaction_hash": tx_hash
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn create_tip_invalid_transaction_hash(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": invalid_tx_hash()
    });

    let response = server.post("/tips").json(&payload).await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn list_tips_success(pool: PgPool) {
    let (server, mock_server) = spawn_app(pool).await;

    let creator_payload = json!({
        "username": "alice",
        "wallet_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    });
    let created = server.post("/creators").json(&creator_payload).await;
    created.assert_status(StatusCode::CREATED);
    let creator_body: Value = created.json();
    let creator_id = creator_body["id"].as_str().unwrap().to_string();

    let tx_hash = valid_tx_hash();
    mount_horizon_success(&mock_server, &tx_hash).await;

    let tip_payload = json!({
        "creator_id": creator_id,
        "amount": "10.0000000",
        "transaction_hash": tx_hash,
        "message": "great work"
    });
    let tip_response = server.post("/tips").json(&tip_payload).await;
    tip_response.assert_status(StatusCode::CREATED);

    let response = server
        .get(&format!("/creators/{}/tips", creator_id))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();
    let tips = body.as_array().expect("expected an array of tips");
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0]["transaction_hash"], tx_hash);
}

#[sqlx::test]
async fn list_tips_not_found(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let missing = Uuid::new_v4();
    let response = server.get(&format!("/creators/{}/tips", missing)).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn unknown_route_returns_404(pool: PgPool) {
    let (server, _mock) = spawn_app(pool).await;

    let response = server.get("/this/route/does/not/exist").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn horizon_mock_is_reachable(pool: PgPool) {
    let (_server, mock_server) = spawn_app(pool).await;

    let tx_hash = valid_tx_hash();
    mount_horizon_success(&mock_server, &tx_hash).await;

    let client = reqwest::Client::new();
    let url = format!("{}/transactions/{}", mock_server.uri(), tx_hash);
    let resp = client.get(&url).send().await.expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// Ensure the path_regex matcher import is used (kept for future flexible matching).
#[allow(dead_code)]
fn _unused_path_regex() {
    let _ = path_regex(".*");
}