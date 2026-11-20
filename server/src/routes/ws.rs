//! `GET /ws`: tick real-time ke browser.
//!
//! Client → server: `{"type":"subscribe","symbol":"AAPL"}` / `{"type":"unsubscribe",...}`
//! Server → client: `{"type":"history","symbol":"AAPL","points":[[t, harga], ...]}`
//!                  (sekali, tepat setelah subscribe berhasil, sebelum trade simbol itu)
//!                  `{"type":"trades","data":[{symbol,price,timestamp,volume}]}`
//!                  `{"type":"error","symbol":"AAPL","message":"..."}`

use std::sync::Arc;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, close_code},
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;

use crate::{
    history::{Point, Recorder},
    hub::{Hub, Subscription},
    market::{Trade, normalize_symbol},
    state::AppState,
    watchlist,
};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMessage {
    Subscribe { symbol: String },
    Unsubscribe { symbol: String },
}

/// Lifetime `'a`: `data` cuma meminjam trade dari batch broadcast, nggak menyalinnya.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ServerMessage<'a> {
    /// Riwayat sparkline 30 menit terakhir (lihat `history.rs`), urut dari yang paling lama.
    History {
        symbol: String,
        points: Vec<Point>,
    },
    Trades {
        data: Vec<&'a Trade>,
    },
    Error {
        symbol: Option<String>,
        message: String,
    },
}

pub async fn ws(upgrade: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    // Koneksi WS yang sudah di-upgrade nggak lagi dilacak graceful shutdown-nya axum,
    // jadi kita lacak sendiri lewat TaskTracker supaya `main` bisa menunggunya.
    let tasks = state.tasks.clone();
    upgrade.on_upgrade(move |socket| {
        tasks.track_future(client(
            socket,
            state.hub,
            state.recorder,
            state.db,
            state.shutdown,
        ))
    })
}

/// Satu task per browser. Selesai saat browser menutup koneksi, kirim gagal, atau server
/// shutdown; `subscription` di-drop di akhir dan melepas semua simbolnya.
async fn client(
    mut socket: WebSocket,
    hub: Arc<Hub>,
    recorder: Arc<Recorder>,
    db: PgPool,
    shutdown: CancellationToken,
) {
    let _client = crate::metrics::WsClient::connected();
    let mut trades = hub.trades();
    let mut subscription = hub.subscription();
    tracing::debug!("ws client terhubung");

    loop {
        tokio::select! {
            () = shutdown.cancelled() => {
                // 1001 "going away": browser tahu server pergi dengan sengaja.
                // useLiveTicks tetap reconnect dengan backoff seperti biasa.
                let frame = CloseFrame { code: close_code::AWAY, reason: "server restart".into() };
                let _ = socket.send(Message::Close(Some(frame))).await;
                break;
            }
            incoming = socket.recv() => {
                // `None` = koneksi selesai, `Err` = koneksi rusak. Dua-duanya: berhenti.
                let Some(Ok(message)) = incoming else { break };
                // Close/Ping/Pong/Binary diabaikan; Ping dijawab otomatis oleh axum.
                let Message::Text(text) = message else { continue };
                // Riwayat dikirim di sini, sebelum loop sempat meneruskan trade simbol itu:
                // satu task, satu koneksi, jadi urutannya pasti.
                let reply = match handle_command(text.as_str(), &mut subscription, &db).await {
                    Ok(Some(symbol)) => Some(ServerMessage::History {
                        points: recorder.points(&symbol),
                        symbol,
                    }),
                    Ok(None) => None,
                    Err(error) => Some(error),
                };
                if let Some(reply) = reply
                    && send(&mut socket, &reply).await.is_err()
                {
                    break;
                }
            }
            batch = trades.recv() => match batch {
                Ok(batch) => {
                    let data: Vec<&Trade> = batch
                        .iter()
                        .filter(|trade| subscription.contains(&trade.symbol))
                        .collect();
                    if !data.is_empty() && send(&mut socket, &ServerMessage::Trades { data }).await.is_err() {
                        break;
                    }
                }
                // Client terlalu lambat dan ketinggalan. Untuk harga live, tick lama nggak
                // berguna lagi: lewati saja dan lanjut dari yang terbaru.
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "ws client tertinggal, tick dilewati");
                }
                Err(RecvError::Closed) => break,
            },
        }
    }
    tracing::debug!("ws client terputus");
}

/// `Ok(Some(simbol))` = subscribe baru berhasil, kirim riwayatnya.
/// `Err` berisi pesan error yang perlu dikirim balik ke client.
///
/// Subscribe cuma untuk simbol di watchlist: batas 50 simbol live berlaku untuk seluruh
/// server, jadi kalau siapa pun boleh subscribe simbol apa saja, satu pengunjung bisa
/// menghabiskan semua slot.
async fn handle_command(
    text: &str,
    subscription: &mut Subscription,
    db: &PgPool,
) -> Result<Option<String>, ServerMessage<'static>> {
    let error = |symbol: Option<String>, message: String| ServerMessage::Error { symbol, message };

    let command: ClientMessage =
        serde_json::from_str(text).map_err(|_| error(None, "pesan tidak dikenal".to_string()))?;

    match command {
        ClientMessage::Subscribe { symbol } => {
            let normalized = normalize_symbol(&symbol)
                .ok_or_else(|| error(Some(symbol.clone()), "simbol tidak valid".to_string()))?;
            match watchlist::contains(db, &normalized).await {
                Ok(true) => {}
                Ok(false) => {
                    return Err(error(
                        Some(normalized),
                        "nggak ada di watchlist".to_string(),
                    ));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "ws: gagal cek watchlist");
                    return Err(error(Some(normalized), "kesalahan server".to_string()));
                }
            }
            match subscription.add(&normalized) {
                Ok(()) => Ok(Some(normalized)),
                Err(err) => Err(error(Some(normalized), err.to_string())),
            }
        }
        ClientMessage::Unsubscribe { symbol } => {
            if let Some(normalized) = normalize_symbol(&symbol) {
                subscription.remove(&normalized);
            }
            Ok(None)
        }
    }
}

async fn send(socket: &mut WebSocket, message: &ServerMessage<'_>) -> Result<(), axum::Error> {
    // Serialisasi enum + &Trade nggak mungkin gagal.
    let json = serde_json::to_string(message).expect("pesan selalu bisa di-serialize");
    socket.send(Message::Text(json.into())).await
}
