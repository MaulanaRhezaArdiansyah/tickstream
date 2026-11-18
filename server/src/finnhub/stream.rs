//! Satu koneksi WebSocket ke Finnhub untuk seluruh server.
//!
//! Pola actor: satu task memegang koneksi dan daftar simbol. Bagian lain server cuma
//! mengirim perintah lewat channel, jadi nggak ada state bersama dan nggak perlu `Mutex`.

use std::{collections::BTreeSet, time::Duration};

use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::market::Trade;

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

enum Command {
    Subscribe(String),
}

/// Pegangan untuk bicara ke task stream. Murah di-clone: isinya cuma ujung kirim channel.
/// Kalau semua pegangan di-drop, task-nya ikut berhenti.
#[derive(Clone)]
pub struct FinnhubStream {
    commands: mpsc::Sender<Command>,
}

impl FinnhubStream {
    /// Jalankan task upstream di background dan kembalikan pegangannya.
    pub fn spawn(api_key: &SecretString) -> Self {
        // Task butuh salinan key miliknya sendiri, karena ia hidup lebih lama dari pemanggil.
        let api_key = SecretString::from(api_key.expose_secret().to_owned());
        let (commands, rx) = mpsc::channel(64);
        tokio::spawn(run(api_key, rx));
        Self { commands }
    }

    pub async fn subscribe(&self, symbol: &str) {
        // Kirim cuma gagal kalau task sudah berhenti; di titik itu nggak ada yang bisa dilakukan.
        let _ = self
            .commands
            .send(Command::Subscribe(symbol.to_string()))
            .await;
    }
}

/// Loop utama actor: buka sesi, kalau putus tunggu (backoff) lalu buka lagi.
async fn run(api_key: SecretString, mut commands: mpsc::Receiver<Command>) {
    // Daftar simbol yang *diinginkan*. Hidup di luar sesi supaya selamat melewati reconnect.
    let mut symbols = BTreeSet::new();
    let mut backoff = MIN_BACKOFF;

    loop {
        match session(&api_key, &mut symbols, &mut commands, &mut backoff).await {
            Ok(()) => {
                tracing::info!("finnhub stream berhenti: semua pegangan sudah di-drop");
                return;
            }
            Err(err) => {
                tracing::warn!(error = %format!("{err:#}"), retry_in = ?backoff, "finnhub stream putus");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// Satu sesi koneksi. `Ok(())` berarti server shutdown (channel perintah tertutup);
/// `Err` berarti koneksi gagal atau putus dan perlu dibuka ulang.
async fn session(
    api_key: &SecretString,
    symbols: &mut BTreeSet<String>,
    commands: &mut mpsc::Receiver<Command>,
    backoff: &mut Duration,
) -> anyhow::Result<()> {
    // Finnhub cuma menerima token lewat query string di WS. URL ini jangan pernah di-log.
    let url = format!("{WS_URL}?token={}", api_key.expose_secret());
    let (ws, _response) = connect_async(url).await.context("gagal konek")?;
    let (mut sink, mut stream) = ws.split();
    let connected_at = tokio::time::Instant::now();
    tracing::info!(symbols = symbols.len(), "finnhub stream terhubung");

    // Koneksi baru belum tahu apa-apa: kirim ulang semua simbol yang masih diinginkan.
    for symbol in symbols.iter() {
        sink.send(subscribe_message(symbol)).await?;
    }

    loop {
        // Tunggu mana yang duluan: perintah dari server, atau pesan dari Finnhub.
        tokio::select! {
            command = commands.recv() => match command {
                None => return Ok(()),
                Some(Command::Subscribe(symbol)) => {
                    // Catat dulu sebagai keinginan, baru kirim: kalau kirimnya gagal,
                    // simbol ini tetap ikut di-subscribe ulang setelah reconnect.
                    if symbols.insert(symbol.clone()) {
                        sink.send(subscribe_message(&symbol)).await?;
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
                handle_message(message)?;
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

fn handle_message(message: Message) -> anyhow::Result<()> {
    match message {
        Message::Text(text) => match serde_json::from_str::<Upstream>(&text) {
            Ok(Upstream::Trade { data }) => {
                for trade in data {
                    tracing::debug!(
                        symbol = %trade.symbol,
                        price = trade.price,
                        volume = trade.volume,
                        ts_ms = trade.timestamp,
                        "trade"
                    );
                }
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

fn subscribe_message(symbol: &str) -> Message {
    Message::text(serde_json::json!({ "type": "subscribe", "symbol": symbol }).to_string())
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
