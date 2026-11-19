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
    Ok(handle)
}

/// Teks `# HELP` yang ikut dirender di atas tiap metrik.
fn describe() {
    ::metrics::describe_gauge!(START_TIME, "Waktu server mulai (unix detik)");
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
#[cfg(test)]
pub mod testing {
    use std::{cell::RefCell, collections::HashMap};

    use metrics_util::{
        MetricKind,
        debugging::{DebugValue, DebuggingRecorder, Snapshotter},
    };

    /// (jenis, nama, label terurut): identitas satu deret metrik.
    type SeriesKey = (MetricKind, String, Vec<(String, String)>);

    /// Metrik yang tertangkap sejak `capture()` sampai nilai ini di-drop.
    pub struct Captured {
        snapshotter: Snapshotter,
        /// `snapshot()` mengosongkan recorder, jadi hasil tiap snapshot dikumpulkan di sini.
        seen: RefCell<HashMap<SeriesKey, DebugValue>>,
        _guard: ::metrics::LocalRecorderGuard<'static>,
    }

    /// Pasang recorder lokal untuk thread ini. Test tokio/sqlx berjalan di runtime satu
    /// thread, jadi task yang di-spawn test (server, koneksi WS) ikut tercatat di sini,
    /// sementara test lain yang jalan paralel di thread lain nggak.
    pub fn capture() -> Captured {
        // Dibocorkan supaya bisa dipinjam `'static` oleh guard; cuma di test, ukurannya kecil.
        let recorder: &'static DebuggingRecorder = Box::leak(Box::new(DebuggingRecorder::new()));
        Captured {
            snapshotter: recorder.snapshotter(),
            seen: RefCell::default(),
            _guard: ::metrics::set_default_local_recorder(recorder),
        }
    }

    fn series<'a>(
        kind: MetricKind,
        name: &str,
        labels: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> SeriesKey {
        let mut labels: Vec<_> = labels
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        labels.sort_unstable();
        (kind, name.to_string(), labels)
    }

    impl Captured {
        fn find<T>(
            &self,
            kind: MetricKind,
            name: &str,
            labels: &[(&str, &str)],
            read: impl FnOnce(&DebugValue) -> T,
        ) -> Option<T> {
            let mut seen = self.seen.borrow_mut();
            for (key, _, _, value) in self.snapshotter.snapshot().into_vec() {
                let labels = key.key().labels().map(|l| (l.key(), l.value()));
                let id = series(key.kind(), key.key().name(), labels);
                // Counter dan histogram dijumlahkan dengan yang sudah terlihat; gauge diganti.
                let merged = match (seen.remove(&id), value) {
                    (Some(DebugValue::Counter(a)), DebugValue::Counter(b)) => {
                        DebugValue::Counter(a + b)
                    }
                    (Some(DebugValue::Histogram(mut a)), DebugValue::Histogram(b)) => {
                        a.extend(b);
                        DebugValue::Histogram(a)
                    }
                    (_, value) => value,
                };
                seen.insert(id, merged);
            }
            seen.get(&series(kind, name, labels.iter().copied()))
                .map(read)
        }

        pub fn counter(&self, name: &str, labels: &[(&str, &str)]) -> u64 {
            self.find(MetricKind::Counter, name, labels, |value| match value {
                DebugValue::Counter(n) => *n,
                _ => 0,
            })
            .unwrap_or(0)
        }

        pub fn histogram_len(&self, name: &str, labels: &[(&str, &str)]) -> usize {
            self.find(MetricKind::Histogram, name, labels, |value| match value {
                DebugValue::Histogram(samples) => samples.len(),
                _ => 0,
            })
            .unwrap_or(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
