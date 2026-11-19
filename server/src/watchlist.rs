//! Akses tabel `watchlist`.
//!
//! `query!` / `query_scalar!` dicek saat COMPILE terhadap skema database: salah nama kolom
//! atau salah tipe jadi error build, bukan error runtime. Tanpa database yang hidup, build
//! memakai hasil `cargo sqlx prepare` di folder `.sqlx`.

use sqlx::PgPool;

/// Sama dengan batas simbol live di Hub; lebih dari ini nggak bisa di-stream semua.
pub const MAX_ENTRIES: i64 = crate::hub::MAX_SYMBOLS as i64;

pub async fn list(db: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!("SELECT symbol FROM watchlist ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn contains(db: &PgPool, symbol: &str) -> sqlx::Result<bool> {
    // `as "exists!"`: bilang ke sqlx kolom ini pasti nggak NULL, jadi tipenya `bool`
    // (bukan `Option<bool>`).
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM watchlist WHERE symbol = $1) AS "exists!""#,
        symbol
    )
    .fetch_one(db)
    .await
}

pub async fn count(db: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM watchlist"#)
        .fetch_one(db)
        .await
}

/// `false` kalau simbolnya sudah ada.
pub async fn add(db: &PgPool, symbol: &str) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "INSERT INTO watchlist (symbol) VALUES ($1) ON CONFLICT (symbol) DO NOTHING",
        symbol
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// `false` kalau simbolnya memang nggak ada.
pub async fn remove(db: &PgPool, symbol: &str) -> sqlx::Result<bool> {
    let result = sqlx::query!("DELETE FROM watchlist WHERE symbol = $1", symbol)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}
