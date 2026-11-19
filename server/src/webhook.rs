//! Akses tabel `webhook_events`.

use sqlx::PgPool;

/// Simpan satu event mentah, kembalikan id-nya. `payload` harus JSON valid
/// (dicek di handler); Postgres yang mengubahnya jadi JSONB.
pub async fn store(db: &PgPool, payload: &str) -> Result<i64, sqlx::Error> {
    // `$1::text` lalu `::jsonb`: parameter dikirim sebagai teks biasa, jadi nggak perlu
    // fitur `json` sqlx. JSONB menyimpan isinya utuh, tapi spasi dan urutan key dinormalisasi.
    sqlx::query_scalar!(
        "INSERT INTO webhook_events (payload) VALUES (($1::text)::jsonb) RETURNING id",
        payload
    )
    .fetch_one(db)
    .await
}
