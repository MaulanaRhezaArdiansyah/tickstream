//! Satu koneksi WebSocket ke Finnhub untuk seluruh server.
//!
//! Pola actor: satu task memegang koneksi dan daftar simbol. Bagian lain server cuma
//! mengirim perintah lewat channel, jadi nggak ada state bersama dan nggak perlu `Mutex`.
//!
//! Free tier efektifnya cuma boleh SATU koneksi WS per API key. Dua proses dengan key yang
//! sama (mis. dua `cargo run`) bakal saling tendang: koneksi di-reset tiap beberapa detik.

use std::{collections::BTreeSet, time::Duration};

use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Message,
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::market::{Trade, TradeBatch, TradeFeed};

/// Garis miring di akhir itu wajib. Tanpa path, tungstenite menulis `GET ?token=...`
/// (bukan `GET /?token=...`) dan nginx Finnhub membalas 400.
const WS_URL: &str = "wss://ws.finnhub.io/";
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// Finnhub mengirim `{"type":"ping"}` kalau lagi sepi trade (teramati ~20 detik setelah
/// konek, intervalnya nggak terdokumentasi). Kalau selama ini nggak ada pesan sama sekali,
/// koneksi dianggap mati diam-diam (half-open) lalu dibuka ulang.
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Finnhub kadang me-reset koneksi baru beberapa detik setelah tersambung. Backoff baru
/// dikembalikan ke minimum kalau koneksi sudah bertahan selama ini; tanpa ini, pola
/// "konek, dapat 1 pesan, di-reset" bikin reconnect tiap detik dan berujung 429.
const STABLE_AFTER: Duration = Duration::from_secs(30);
/// Saat shutdown, berapa lama menunggu Finnhub membalas close frame kita.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

enum Command {
    Subscribe(String),
    Unsubscribe(String),
}

/// Pegangan untuk bicara ke task stream. Kalau pegangan di-drop, task-nya ikut berhenti.
///
/// Channel-nya unbounded supaya kirim perintah bisa sinkron: unsubscribe dipanggil dari
/// `Drop`, yang nggak bisa `.await`. Aman karena perintah cuma muncul saat jumlah pemakai
/// simbol berubah 0↔1, dan simbol dibatasi 50.
pub struct FinnhubStream {
    commands: mpsc::UnboundedSender<Command>,
}

impl FinnhubStream {
    /// Jalankan task upstream di background. Trade yang masuk dikirim ke `trades`.
    /// Task berhenti (dan menutup koneksi dengan sopan) saat `shutdown` di-cancel;
    /// `tasks` dipakai `main` untuk menunggunya selesai.
    pub fn spawn(
        api_key: &SecretString,
        trades: broadcast::Sender<TradeBatch>,
        shutdown: CancellationToken,
        tasks: &TaskTracker,
    ) -> Self {
        // Task butuh salinan key miliknya sendiri, karena ia hidup lebih lama dari pemanggil.
        let api_key = SecretString::from(api_key.expose_secret().to_owned());
        let (commands, rx) = mpsc::unbounded_channel();
        tasks.spawn(run(api_key, rx, trades, shutdown));
        Self { commands }
    }

    fn send(&self, command: Command) {
        // Kirim cuma gagal kalau task sudah berhenti; di titik itu nggak ada yang bisa dilakukan.
        let _ = self.commands.send(command);
    }
}

impl TradeFeed for FinnhubStream {
    fn subscribe(&self, symbol: &str) {
        self.send(Command::Subscribe(symbol.to_string()));
    }

    fn unsubscribe(&self, symbol: &str) {
        self.send(Command::Unsubscribe(symbol.to_string()));
    }
}

/// Loop utama actor: buka sesi, kalau putus tunggu (backoff) lalu buka lagi.
async fn run(
    api_key: SecretString,
    mut commands: mpsc::UnboundedReceiver<Command>,
    trades: broadcast::Sender<TradeBatch>,
    shutdown: CancellationToken,
) {
    // Daftar simbol yang *diinginkan*. Hidup di luar sesi supaya selamat melewati reconnect.
    let mut symbols = BTreeSet::new();
    let mut backoff = MIN_BACKOFF;

    loop {
        // Belum ada yang minta simbol apa pun: jangan buka koneksi dulu. Jatahnya cuma satu
        // koneksi per key, jadi server yang nggak sedang dipakai browser nggak ikut memakainya.
        if symbols.is_empty() {
            let command = tokio::select! {
                command = commands.recv() => command,
                () = shutdown.cancelled() => return,
            };
            match command {
                None => return,
                Some(Command::Subscribe(symbol)) => {
                    symbols.insert(symbol);
                }
                Some(Command::Unsubscribe(_)) => {}
            }
            continue;
        }

        match session(
            &api_key,
            &mut symbols,
            &mut commands,
            &trades,
            &mut backoff,
            &shutdown,
        )
        .await
        {
            Ok(()) => {
                tracing::info!("finnhub stream berhenti");
                return;
            }
            Err(err) => {
                tracing::warn!(error = %format!("{err:#}"), retry_in = ?backoff, "finnhub stream putus");
                tokio::select! {
                    () = tokio::time::sleep(backoff) => {}
                    () = shutdown.cancelled() => return,
                }
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// Satu sesi koneksi. `Ok(())` berarti server shutdown (token di-cancel atau channel tertutup);
/// `Err` berarti koneksi gagal atau putus dan perlu dibuka ulang.
async fn session(
    api_key: &SecretString,
    symbols: &mut BTreeSet<String>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    trades: &broadcast::Sender<TradeBatch>,
    backoff: &mut Duration,
    shutdown: &CancellationToken,
) -> anyhow::Result<()> {
    // Finnhub cuma menerima token lewat query string di WS. URL ini jangan pernah di-log.
    let url = format!("{WS_URL}?token={}", api_key.expose_secret());
    let (ws, _response) = connect_async(url).await.context("gagal konek")?;
    let (mut sink, mut stream) = ws.split();
    let connected_at = tokio::time::Instant::now();
    tracing::info!(symbols = symbols.len(), "finnhub stream terhubung");

    // Koneksi baru belum tahu apa-apa: kirim ulang semua simbol yang masih diinginkan.
    for symbol in symbols.iter() {
        sink.send(control_message("subscribe", symbol)).await?;
    }

    loop {
        // Tunggu mana yang duluan: perintah dari server, atau pesan dari Finnhub.
        tokio::select! {
            () = shutdown.cancelled() => {
                // Pamit dengan close frame. Tanpa ini Finnhub cuma melihat TCP putus dan
                // (sepertinya) masih menganggap koneksi lama hidup, sehingga koneksi dari
                // proses yang baru di-restart sempat di-reset.
                let goodbye = CloseFrame { code: CloseCode::Normal, reason: "server shutdown".into() };
                sink.send(Message::Close(Some(goodbye))).await?;
                // Tunggu balasan close dari Finnhub sebentar supaya handshake-nya tuntas.
                let _ = tokio::time::timeout(CLOSE_TIMEOUT, async {
                    while let Some(Ok(message)) = stream.next().await {
                        if message.is_close() {
                            break;
                        }
                    }
                })
                .await;
                return Ok(());
            }
            command = commands.recv() => match command {
                None => return Ok(()),
                Some(Command::Subscribe(symbol)) => {
                    // Catat dulu sebagai keinginan, baru kirim: kalau kirimnya gagal,
                    // simbol ini tetap ikut di-subscribe ulang setelah reconnect.
                    if symbols.insert(symbol.clone()) {
                        tracing::debug!(%symbol, "upstream subscribe");
                        sink.send(control_message("subscribe", &symbol)).await?;
                    }
                }
                Some(Command::Unsubscribe(symbol)) => {
                    if symbols.remove(&symbol) {
                        tracing::debug!(%symbol, "upstream unsubscribe");
                        sink.send(control_message("unsubscribe", &symbol)).await?;
                    }
                }
            },
            // Timer idle dibuat ulang tiap putaran loop, jadi perintah yang masuk juga
            // me-reset-nya. Nggak masalah: perintah jarang, ping Finnhub sering.
            next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => {
                let message = match next {
                    Err(_) => bail!("tidak ada pesan selama {IDLE_TIMEOUT:?}"),
                    Ok(None) => bail!("koneksi ditutup"),
                    Ok(Some(message)) => message.context("gagal membaca pesan")?,
                };
                if connected_at.elapsed() >= STABLE_AFTER {
                    *backoff = MIN_BACKOFF;
                }
                handle_message(message, trades)?;
            }
        }
    }
}

/// Pesan dari Finnhub. `tag = "type"`: serde memilih varian dari field `type` di JSON.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Upstream {
    Trade { data: Vec<Trade> },
    Ping,
    Error { msg: String },
}

fn handle_message(message: Message, trades: &broadcast::Sender<TradeBatch>) -> anyhow::Result<()> {
    match message {
        Message::Text(text) => match serde_json::from_str::<Upstream>(&text) {
            Ok(Upstream::Trade { data }) => {
                tracing::trace!(count = data.len(), "trades");
                // Gagal cuma kalau belum ada client yang mendengarkan; trade-nya memang dibuang.
                let _ = trades.send(data.into());
            }
            Ok(Upstream::Ping) => tracing::trace!("ping"),
            Ok(Upstream::Error { msg }) => tracing::warn!(%msg, "finnhub stream error"),
            Err(err) => tracing::warn!(error = %err, "pesan finnhub tidak dikenal"),
        },
        Message::Close(frame) => bail!("ditutup oleh Finnhub: {frame:?}"),
        // Ping/Pong level WebSocket dijawab otomatis oleh tungstenite.
        _ => {}
    }
    Ok(())
}

fn control_message(kind: &str, symbol: &str) -> Message {
    Message::text(serde_json::json!({ "type": kind, "symbol": symbol }).to_string())
}

#[cfg(test)]
mod tests {
    use super::Upstream;

    fn parse(json: &str) -> Upstream {
        serde_json::from_str(json).expect("harusnya bisa di-parse")
    }

    #[test]
    fn parses_trade_batch() {
        let json = r#"{"type":"trade","data":[
            {"s":"AAPL","p":191.5,"t":1700000000123,"v":100,"c":null},
            {"s":"BINANCE:BTCUSDT","p":7296.89,"t":1700000000456,"v":0.011467}
        ]}"#;
        let Upstream::Trade { data } = parse(json) else {
            panic!("harusnya varian Trade");
        };
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].symbol, "AAPL");
        assert_eq!(data[0].timestamp, 1_700_000_000_123);
        assert_eq!(data[1].volume, 0.011467);
    }

    #[test]
    fn parses_ping_and_error() {
        assert!(matches!(parse(r#"{"type":"ping"}"#), Upstream::Ping));
        assert!(matches!(
            parse(r#"{"type":"error","msg":"Invalid symbol"}"#),
            Upstream::Error { msg } if msg == "Invalid symbol"
        ));
    }

    #[test]
    fn rejects_unknown_type() {
        assert!(serde_json::from_str::<Upstream>(r#"{"type":"news","data":[]}"#).is_err());
    }
}
