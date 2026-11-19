//! Test route lewat `Router` langsung (tanpa buka port): request dibuat di memori,
//! dikirim pakai `oneshot`, response dibaca balik.
//!
//! `#[sqlx::test]` membuat database baru per test, menjalankan migration (jadi seed
//! watchlist ikut ada), lalu membuangnya. Butuh Postgres yang jalan dan `DATABASE_URL`.
//! Finnhub diganti `FakeMarket`: test nggak boleh bergantung pada jaringan atau kuota API.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tower::ServiceExt;

use crate::{
    hub::Hub,
    market::{MarketDataSource, MarketError, MarketStatus, Profile, Quote, TradeFeed},
    state::AppState,
};

/// Ticker yang "dikenal" sumber data palsu. Sisanya dianggap nggak ada.
const KNOWN: &[&str] = &["AAPL", "NVDA", "AMD"];

struct FakeMarket;

#[async_trait]
impl MarketDataSource for FakeMarket {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
        if !KNOWN.contains(&symbol) {
            return Err(MarketError::UnknownSymbol(symbol.to_string()));
        }
        Ok(Quote {
            price: 100.0,
            open: 99.0,
            high: 101.0,
            low: 98.0,
            prev_close: 99.5,
            timestamp: 0,
        })
    }

    async fn profile(&self, symbol: &str) -> Result<Profile, MarketError> {
        Ok(Profile {
            name: format!("{symbol} Inc"),
        })
    }

    async fn market_status(&self) -> Result<MarketStatus, MarketError> {
        Ok(MarketStatus {
            is_open: false,
            session: None,
        })
    }
}

/// Route watchlist nggak menyentuh stream; feed ini cuma pengisi slot.
struct NoFeed;

impl TradeFeed for NoFeed {
    fn subscribe(&self, _symbol: &str) {}
    fn unsubscribe(&self, _symbol: &str) {}
}

fn app(db: PgPool) -> Router {
    let (trades, _) = broadcast::channel(1);
    super::router(AppState {
        market: Arc::new(FakeMarket),
        hub: Arc::new(Hub::new(Arc::new(NoFeed), trades)),
        db,
        shutdown: CancellationToken::new(),
        tasks: TaskTracker::new(),
    })
}

/// Kirim satu request, kembalikan status + body JSON (`Null` kalau body kosong).
async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();

    // `oneshot` memakai service-nya, makanya clone. Router murah di-clone (isinya Arc).
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

async fn list(app: &Router) -> Vec<String> {
    let (status, body) = call(app, "GET", "/api/watchlist", None).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_value(body).unwrap()
}

#[sqlx::test]
async fn lists_seeded_symbols_in_order(db: PgPool) {
    let symbols = list(&app(db)).await;
    assert_eq!(symbols.len(), 9);
    assert_eq!(symbols.first().map(String::as_str), Some("AAPL"));
    assert_eq!(symbols.last().map(String::as_str), Some("BINANCE:BTCUSDT"));
}

#[sqlx::test]
async fn adds_normalized_symbol_at_the_end(db: PgPool) {
    let app = app(db);
    let (status, body) = call(
        &app,
        "POST",
        "/api/watchlist",
        Some(json!({"symbol": " amd "})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body, json!("AMD"));
    assert_eq!(list(&app).await.last().map(String::as_str), Some("AMD"));
}

#[sqlx::test]
async fn rejects_bad_additions(db: PgPool) {
    let app = app(db);
    let cases = [
        (json!({"symbol": "nvda"}), StatusCode::CONFLICT), // sudah ada
        (json!({"symbol": "xyz"}), StatusCode::NOT_FOUND), // ticker nggak dikenal
        (json!({"symbol": "bad;x"}), StatusCode::BAD_REQUEST), // format salah
        (json!({"sym": 1}), StatusCode::BAD_REQUEST),      // body salah bentuk
    ];
    for (body, expected) in cases {
        let (status, reply) = call(&app, "POST", "/api/watchlist", Some(body.clone())).await;
        assert_eq!(status, expected, "body {body}");
        assert!(
            reply["error"].is_string(),
            "error harus JSON untuk body {body}: {reply}"
        );
    }
    assert_eq!(list(&app).await.len(), 9, "nggak ada yang boleh tersimpan");
}

#[sqlx::test]
async fn rejects_addition_when_full(db: PgPool) {
    // Isi sampai 50 langsung lewat SQL (9 seed + 41).
    sqlx::query("INSERT INTO watchlist (symbol) SELECT 'ZZ' || g FROM generate_series(1, 41) g")
        .execute(&db)
        .await
        .unwrap();
    let app = app(db);
    let (status, body) = call(
        &app,
        "POST",
        "/api/watchlist",
        Some(json!({"symbol": "amd"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("penuh"));
}

#[sqlx::test]
async fn removes_symbol_once(db: PgPool) {
    let app = app(db);
    let (status, _) = call(&app, "DELETE", "/api/watchlist/nvda", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!list(&app).await.contains(&"NVDA".to_string()));

    let (status, _) = call(&app, "DELETE", "/api/watchlist/nvda", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn removes_url_encoded_symbol(db: PgPool) {
    let app = app(db);
    let (status, _) = call(&app, "DELETE", "/api/watchlist/BINANCE%3ABTCUSDT", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn unknown_route_is_json_404(db: PgPool) {
    let (status, body) = call(&app(db), "GET", "/api/nggak-ada", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string());
}
