use std::sync::Arc;

use sqlx::PgPool;

use crate::{hub::Hub, market::MarketDataSource};

/// State yang dibagi ke semua handler. Axum meng-`clone()` ini per request,
/// jadi isinya `Arc`: yang ke-copy cuma pointer + counter, bukan datanya.
#[derive(Clone)]
pub struct AppState {
    pub market: Arc<dyn MarketDataSource>,
    pub hub: Arc<Hub>,
    /// `PgPool` sudah berisi `Arc` di dalamnya, jadi murah di-clone.
    pub db: PgPool,
}
