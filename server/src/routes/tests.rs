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
    market::{
        MarketDataSource, MarketError, MarketStatus, Profile, Quote, SymbolInfo, SymbolKind,
        TradeFeed,
    },
    state::AppState,
};

/// Token pemilik di semua test, kecuali yang sengaja menguji tanpa token.
const ADMIN: &str = "token-admin-untuk-test-minimal-32-karakter";

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
            logo: None,
        })
    }

    async fn market_status(&self) -> Result<MarketStatus, MarketError> {
        Ok(MarketStatus {
            is_open: false,
            session: None,
        })
    }

    async fn us_symbols(&self) -> Result<Arc<[SymbolInfo]>, MarketError> {
        Ok(Arc::new([
            SymbolInfo {
                symbol: "AAPL".into(),
                name: "APPLE INC".into(),
                kind: SymbolKind::Stock,
            },
            SymbolInfo {
                symbol: "SPY".into(),
                name: "SPDR S&P 500 ETF".into(),
                kind: SymbolKind::Etf,
            },
        ]))
    }
}

/// Route watchlist nggak menyentuh stream; feed ini cuma pengisi slot.
struct NoFeed;

impl TradeFeed for NoFeed {
    fn subscribe(&self, _symbol: &str) {}
    fn unsubscribe(&self, _symbol: &str) {}
}

fn state(db: PgPool) -> AppState {
    let (trades, _) = broadcast::channel(1);
    AppState {
        market: Arc::new(FakeMarket),
        hub: Arc::new(Hub::new(Arc::new(NoFeed), trades)),
        db,
        shutdown: CancellationToken::new(),
        tasks: TaskTracker::new(),
        webhook_secret: None,
        admin_token: Some(Arc::new(ADMIN.into())),
    }
}

fn app(db: PgPool) -> Router {
    super::router(state(db), None)
}

/// Kirim satu request sebagai pemilik, kembalikan status + body JSON (`Null` kalau kosong).
async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    call_as(app, Some(ADMIN), method, uri, body).await
}

/// Seperti `call`, tapi token-nya bisa diganti atau dihilangkan (`None` = pengunjung).
async fn call_as(
    app: &Router,
    token: Option<&str>,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
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

// ---- halaman web (production) ----

#[sqlx::test]
async fn serves_the_web_build_next_to_the_api(db: PgPool) {
    let dir = std::env::temp_dir().join(format!("tickstream-web-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<p>halaman</p>").unwrap();
    std::fs::write(dir.join("assets/app.js"), "console.log(1)").unwrap();
    let app = super::router(state(db), Some(&dir));

    let get = |uri: &'static str| {
        let app = app.clone();
        async move {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let response = app.oneshot(request).await.unwrap();
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            (status, String::from_utf8(bytes.to_vec()).unwrap())
        }
    };

    assert_eq!(get("/").await, (StatusCode::OK, "<p>halaman</p>".into()));
    assert_eq!(
        get("/assets/app.js").await,
        (StatusCode::OK, "console.log(1)".into())
    );
    assert_eq!(
        get("/sembarang").await.1,
        "<p>halaman</p>",
        "path lain dapat index.html"
    );

    let (status, body) = get("/api/nggak-ada").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("\"error\""), "API tetap JSON: {body}");
    let (status, _) = get("/api/watchlist").await;
    assert_eq!(status, StatusCode::OK);

    std::fs::remove_dir_all(&dir).ok();
}

// ---- pemilik vs pengunjung ----

#[sqlx::test]
async fn visitors_can_read_but_not_change_the_watchlist(db: PgPool) {
    let app = app(db);
    let (status, _) = call_as(&app, None, "GET", "/api/watchlist", None).await;
    assert_eq!(status, StatusCode::OK);

    for token in [None, Some("salah"), Some(&ADMIN[1..])] {
        let add = Some(json!({"symbol": "amd"}));
        let (status, body) = call_as(&app, token, "POST", "/api/watchlist", add).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "tambah dengan {token:?}");
        assert!(body["error"].is_string());

        let (status, _) = call_as(&app, token, "DELETE", "/api/watchlist/nvda", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "hapus dengan {token:?}");
    }
    assert_eq!(list(&app).await.len(), 9, "watchlist harus utuh");
}

#[sqlx::test]
async fn nobody_can_write_without_configured_token(db: PgPool) {
    let mut state = state(db);
    state.admin_token = None;
    let app = super::router(state, None);
    let (status, _) = call(
        &app,
        "POST",
        "/api/watchlist",
        Some(json!({"symbol": "amd"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn admin_check_accepts_only_the_right_token(db: PgPool) {
    let app = app(db);
    let (status, _) = call(&app, "GET", "/api/admin", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call_as(&app, Some("salah"), "GET", "/api/admin", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn quotes_are_served_only_for_watched_symbols(db: PgPool) {
    let app = app(db);
    let (status, _) = call_as(&app, None, "GET", "/api/quote?symbol=aapl", None).await;
    assert_eq!(status, StatusCode::OK);
    // AMD dikenal sumber data, tapi nggak ada di watchlist seed.
    for uri in ["/api/quote?symbol=AMD", "/api/profile?symbol=AMD"] {
        let (status, _) = call_as(&app, None, "GET", uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

// ---- daftar simbol ----

#[sqlx::test]
async fn lists_symbols_with_cache_header(db: PgPool) {
    let (status, body) = call(&app(db), "GET", "/api/symbols", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!([
            {"symbol": "AAPL", "name": "APPLE INC", "kind": "stock"},
            {"symbol": "SPY", "name": "SPDR S&P 500 ETF", "kind": "etf"},
        ])
    );
}

#[sqlx::test]
async fn symbols_are_gzipped_and_cacheable(db: PgPool) {
    let request = Request::builder()
        .uri("/api/symbols")
        .header(header::ACCEPT_ENCODING, "gzip")
        .body(Body::empty())
        .unwrap();
    let response = app(db).oneshot(request).await.unwrap();
    let headers = response.headers();
    assert_eq!(headers[header::CONTENT_ENCODING], "gzip");
    assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=3600");
}

// ---- webhook ----

const SECRET: &str = "rahasia-test";

/// Kirim body mentah ke webhook dengan header secret opsional.
async fn post_webhook(app: &Router, secret: Option<&str>, body: &str) -> StatusCode {
    let mut request = Request::builder().method("POST").uri("/webhook/finnhub");
    if let Some(secret) = secret {
        request = request.header("X-Finnhub-Secret", secret);
    }
    let request = request.body(Body::from(body.to_owned())).unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

async fn stored_events(db: &PgPool) -> Vec<Value> {
    sqlx::query_scalar::<_, String>("SELECT payload::text FROM webhook_events ORDER BY id")
        .fetch_all(db)
        .await
        .unwrap()
        .iter()
        .map(|text| serde_json::from_str(text).unwrap())
        .collect()
}

#[sqlx::test]
async fn webhook_stores_valid_event_in_background(db: PgPool) {
    let mut state = state(db.clone());
    state.webhook_secret = Some(Arc::new(SECRET.into()));
    let tasks = state.tasks.clone();
    let app = super::router(state, None);

    let event = json!({"event": "earnings", "data": [{"symbol": "AAPL"}]});
    let status = post_webhook(&app, Some(SECRET), &event.to_string()).await;
    assert_eq!(status, StatusCode::OK);

    // Penyimpanan jalan di background; tunggu task-nya seperti saat shutdown.
    tasks.close();
    tasks.wait().await;
    assert_eq!(stored_events(&db).await, vec![event]);
}

#[sqlx::test]
async fn webhook_rejects_bad_requests_without_storing(db: PgPool) {
    let mut state = state(db.clone());
    state.webhook_secret = Some(Arc::new(SECRET.into()));
    let tasks = state.tasks.clone();
    let app = super::router(state, None);

    let cases = [
        (None, "{}", StatusCode::UNAUTHORIZED), // tanpa header
        (Some("salah"), "{}", StatusCode::UNAUTHORIZED), // secret beda
        (Some("rahasia-tes"), "{}", StatusCode::UNAUTHORIZED), // prefiks secret asli
        (Some(SECRET), "bukan json", StatusCode::BAD_REQUEST), // body rusak
    ];
    for (secret, body, expected) in cases {
        assert_eq!(
            post_webhook(&app, secret, body).await,
            expected,
            "{secret:?} {body}"
        );
    }

    tasks.close();
    tasks.wait().await;
    assert!(stored_events(&db).await.is_empty());
}

#[sqlx::test]
async fn webhook_is_disabled_without_secret(db: PgPool) {
    let status = post_webhook(&app(db), Some(SECRET), "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
