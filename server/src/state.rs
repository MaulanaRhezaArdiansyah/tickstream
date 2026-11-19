use std::sync::Arc;

use secrecy::SecretString;
use sqlx::PgPool;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{hub::Hub, market::MarketDataSource};

/// State yang dibagi ke semua handler. Axum meng-`clone()` ini per request,
/// jadi isinya `Arc`: yang ke-copy cuma pointer + counter, bukan datanya.
#[derive(Clone)]
pub struct AppState {
    pub market: Arc<dyn MarketDataSource>,
    pub hub: Arc<Hub>,
    /// `PgPool` sudah berisi `Arc` di dalamnya, jadi murah di-clone.
    pub db: PgPool,
    /// Di-cancel saat server diminta berhenti (Ctrl+C / SIGTERM).
    pub shutdown: CancellationToken,
    /// Task yang harus ditunggu sebelum proses keluar (koneksi `/ws`, stream upstream).
    pub tasks: TaskTracker,
    /// Secret webhook Finnhub; `None` = webhook dimatikan. `Arc` karena state di-clone per request.
    pub webhook_secret: Option<Arc<SecretString>>,
    /// Token pemilik untuk mengubah watchlist; `None` = read-only.
    pub admin_token: Option<Arc<SecretString>>,
}
