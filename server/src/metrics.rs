//! Metrik server dalam format Prometheus, dibaca pemilik lewat `GET /metrics`.
//!
//! Semua nama metrik ada di file ini. Bagian lain server cuma memanggil fungsi pencatat di
//! bawah, jadi daftar lengkap apa yang diukur (dan label apa saja yang dipakai) terbaca di
//! satu tempat. Label nggak pernah berisi simbol, IP, token, atau API key.
//!
//! Crate `metrics` memisahkan *mencatat* dari *menyimpan*: makro `counter!`/`gauge!` mengirim
//! ke "recorder" yang sedang terpasang. Di production itu recorder Prometheus (dipasang
//! `install`); di test, recorder lokal per test.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{MatchedPath, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

use crate::{auth::Admin, state::AppState};

const START_TIME: &str = "tickstream_start_time_seconds";
const WS_CLIENTS: &str = "tickstream_ws_clients";
const FINNHUB_TRADES: &str = "tickstream_finnhub_trades_total";
const LAST_TRADE: &str = "tickstream_finnhub_last_trade_timestamp_seconds";
const FINNHUB_CONNECTS: &str = "tickstream_finnhub_ws_connects_total";
const CACHE_REQUESTS: &str = "tickstream_cache_requests_total";
const FINNHUB_REQUESTS: &str = "tickstream_finnhub_requests_total";
const HTTP_REQUESTS: &str = "tickstream_http_requests_total";
const HTTP_DURATION: &str = "tickstream_http_request_duration_seconds";

/// Batas bucket lama respons, dalam detik. Tanpa ini exporter merender histogram sebagai
/// summary (kuantil), yang nggak bisa dijumlahkan Prometheus/Grafana.
const DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Pengaturan recorder Prometheus. Dipakai `install` dan test, supaya test merender persis
/// seperti production.
pub fn builder() -> PrometheusBuilder {
    PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Full(HTTP_DURATION.to_string()), DURATION_BUCKETS)
        .expect("daftar bucket nggak kosong")
}

/// Pasang recorder Prometheus untuk seluruh proses. Dipanggil sekali di `main`.
pub fn install() -> anyhow::Result<PrometheusHandle> {
    let handle = builder().install_recorder()?;
    describe();
    record_start_time();
    start_series_at_zero();
    Ok(handle)
}

/// Munculkan baris feed dan klien dengan nilai 0 sejak server hidup. Tanpa ini barisnya
/// baru ada setelah event pertama, jadi "Finnhub belum pernah konek" (misalnya kalah rebutan
/// socket dengan proses lain yang pakai key yang sama) terlihat sama dengan "nggak dicatat".
fn start_series_at_zero() {
    ::metrics::gauge!(WS_CLIENTS).set(0.0);
    ::metrics::counter!(FINNHUB_TRADES).increment(0);
    ::metrics::counter!(FINNHUB_CONNECTS).increment(0);
}

/// Teks `# HELP` yang ikut dirender di atas tiap metrik.
fn describe() {
    ::metrics::describe_gauge!(START_TIME, "Waktu server mulai (unix detik)");
    ::metrics::describe_gauge!(WS_CLIENTS, "Klien WebSocket yang sedang tersambung");
    ::metrics::describe_counter!(FINNHUB_TRADES, "Trade yang masuk dari Finnhub");
    ::metrics::describe_gauge!(
        LAST_TRADE,
        "Kapan trade terakhir masuk (unix detik); tertinggal jauh saat pasar buka = feed macet"
    );
    ::metrics::describe_counter!(
        FINNHUB_CONNECTS,
        "Sesi WebSocket Finnhub yang berhasil dibuka; lebih dari 1 = pernah reconnect"
    );
    ::metrics::describe_counter!(CACHE_REQUESTS, "Pencarian cache per cache, hit atau miss");
    ::metrics::describe_counter!(
        FINNHUB_REQUESTS,
        "Panggilan REST ke Finnhub per endpoint dan hasilnya"
    );
    ::metrics::describe_counter!(HTTP_REQUESTS, "Request HTTP per pola route dan status");
    ::metrics::describe_histogram!(
        HTTP_DURATION,
        ::metrics::Unit::Seconds,
        "Lama respons per pola route"
    );
}

pub fn record_start_time() {
    ::metrics::gauge!(START_TIME).set(unix_now());
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

/// `count` trade baru masuk dari Finnhub.
pub fn trades_received(count: usize) {
    ::metrics::counter!(FINNHUB_TRADES).increment(count as u64);
    ::metrics::gauge!(LAST_TRADE).set(unix_now());
}

/// Sesi WebSocket ke Finnhub baru saja terbuka.
pub fn finnhub_connected() {
    ::metrics::counter!(FINNHUB_CONNECTS).increment(1);
}

/// Satu klien WebSocket yang tersambung. Hitungannya turun lagi saat nilai ini di-drop,
/// jadi apa pun cara task klien berakhir (ditutup browser, error, shutdown), angkanya benar.
pub struct WsClient(());

impl WsClient {
    pub fn connected() -> Self {
        ::metrics::gauge!(WS_CLIENTS).increment(1.0);
        Self(())
    }
}

impl Drop for WsClient {
    fn drop(&mut self) {
        ::metrics::gauge!(WS_CLIENTS).decrement(1.0);
    }
}

/// Satu pencarian di cache `cache` (`quote`, `profile`, `market_status`, `news`, `symbols`).
pub fn cache_lookup(cache: &'static str, hit: bool) {
    let result = if hit { "hit" } else { "miss" };
    ::metrics::counter!(CACHE_REQUESTS, "cache" => cache, "result" => result).increment(1);
}

/// Hasil satu panggilan REST ke Finnhub.
#[derive(Clone, Copy)]
pub enum Outcome {
    Ok,
    /// 429 dari Finnhub: jatah per menit habis di sisi mereka.
    RateLimited,
    /// Limiter kita menolak sebelum memanggil Finnhub, supaya jatahnya nggak tembus.
    LimitedLocally,
    /// Gagal kirim, 401/403/status lain, atau JSON yang nggak bisa dibaca.
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::RateLimited => "rate_limited",
            Self::LimitedLocally => "limited_locally",
            Self::Error => "error",
        }
    }
}

/// Satu panggilan REST ke Finnhub. `endpoint` = path Finnhub (`/quote`), tanpa query.
pub fn finnhub_request(endpoint: &str, outcome: Outcome) {
    ::metrics::counter!(
        FINNHUB_REQUESTS,
        "endpoint" => endpoint.to_string(),
        "outcome" => outcome.as_str()
    )
    .increment(1);
}

/// Middleware: hitung tiap request dan lama responsnya per pola route.
///
/// Labelnya pola route dari axum (`/api/watchlist/{symbol}`), bukan path asli, jadi simbol
/// nggak jadi label dan jumlah deret tetap kecil. Request yang nggak cocok route mana pun
/// (file statis, 404) dikumpulkan di `other`.
pub async fn track_http(request: Request, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("other", MatchedPath::as_str)
        .to_string();
    let started = Instant::now();
    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    ::metrics::counter!(HTTP_REQUESTS, "route" => route.clone(), "status" => status).increment(1);
    ::metrics::histogram!(HTTP_DURATION, "route" => route).record(started.elapsed().as_secs_f64());
    response
}

/// `GET /metrics`: semua metrik dalam format teks Prometheus. Khusus pemilik.
pub async fn render(_: Admin, State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
}

/// Alat test: tangkap metrik yang dicatat selama satu test, lalu baca nilainya.
///
/// Memakai recorder Prometheus yang sama dengan production dan membaca teks hasil render,
/// jadi yang diuji persis yang dilihat pemilik di `/metrics`. Render nggak mengosongkan
/// nilai, jadi boleh dibaca berkali-kali.
#[cfg(test)]
pub mod testing {
    use metrics_exporter_prometheus::{PrometheusHandle, PrometheusRecorder};

    /// Metrik yang tertangkap sejak `capture()` sampai nilai ini di-drop.
    pub struct Captured {
        handle: PrometheusHandle,
        _guard: ::metrics::LocalRecorderGuard<'static>,
    }

    /// Pasang recorder lokal untuk thread ini. Test tokio/sqlx berjalan di runtime satu
    /// thread, jadi task yang di-spawn test (server, koneksi WS) ikut tercatat di sini,
    /// sementara test lain yang jalan paralel di thread lain nggak.
    pub fn capture() -> Captured {
        // Dibocorkan supaya bisa dipinjam `'static` oleh guard; cuma di test, ukurannya kecil.
        let recorder: &'static PrometheusRecorder =
            Box::leak(Box::new(super::builder().build_recorder()));
        Captured {
            handle: recorder.handle(),
            _guard: ::metrics::set_default_local_recorder(recorder),
        }
    }

    impl Captured {
        /// Nilai baris `name{labels} value` di hasil render; `None` kalau belum ada.
        fn value(&self, name: &str, labels: &[(&str, &str)]) -> Option<f64> {
            let mut expected: Vec<String> =
                labels.iter().map(|(k, v)| format!("{k}=\"{v}\"")).collect();
            expected.sort_unstable();
            self.handle.render().lines().find_map(|line| {
                if line.starts_with('#') {
                    return None;
                }
                let (series, value) = line.rsplit_once(' ')?;
                let (series_name, series_labels) = match series.split_once('{') {
                    Some((n, rest)) => (n, rest.trim_end_matches('}')),
                    None => (series, ""),
                };
                let mut actual: Vec<String> = series_labels
                    .split(',')
                    .filter(|l| !l.is_empty())
                    .map(str::to_string)
                    .collect();
                actual.sort_unstable();
                (series_name == name && actual == expected).then(|| value.parse().unwrap())
            })
        }

        pub fn counter(&self, name: &str, labels: &[(&str, &str)]) -> u64 {
            self.value(name, labels).unwrap_or(0.0) as u64
        }

        pub fn gauge(&self, name: &str, labels: &[(&str, &str)]) -> f64 {
            self.value(name, labels).unwrap_or(0.0)
        }

        /// Jumlah sampel histogram (`<name>_count`).
        pub fn histogram_len(&self, name: &str, labels: &[(&str, &str)]) -> usize {
            self.value(&format!("{name}_count"), labels).unwrap_or(0.0) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sesaat setelah deploy, sebelum ada event apa pun: baris feed dan klien sudah ada
    /// dengan nilai 0, supaya "belum pernah konek" beda dari "nggak dicatat".
    #[test]
    fn feed_and_client_series_start_at_zero() {
        let recorder = builder().build_recorder();
        let handle = recorder.handle();
        ::metrics::with_local_recorder(&recorder, start_series_at_zero);
        let text = handle.render();
        for line in [
            "tickstream_ws_clients 0",
            "tickstream_finnhub_trades_total 0",
            "tickstream_finnhub_ws_connects_total 0",
        ] {
            assert!(
                text.lines().any(|l| l == line),
                "nggak ada `{line}` di:\n{text}"
            );
        }
    }

    #[test]
    fn duration_is_a_histogram() {
        let recorder = builder().build_recorder();
        let handle = recorder.handle();
        ::metrics::with_local_recorder(&recorder, || {
            ::metrics::histogram!(HTTP_DURATION, "route" => "/health").record(0.02);
        });
        let text = handle.render();
        assert!(
            text.contains("tickstream_http_request_duration_seconds_bucket"),
            "{text}"
        );
    }
}
