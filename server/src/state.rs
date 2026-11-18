use std::sync::Arc;

use crate::market::MarketDataSource;

/// State yang dibagi ke semua handler. Axum meng-`clone()` ini per request,
/// jadi isinya `Arc`: yang ke-copy cuma pointer + counter, bukan datanya.
#[derive(Clone)]
pub struct AppState {
    pub market: Arc<dyn MarketDataSource>,
}
