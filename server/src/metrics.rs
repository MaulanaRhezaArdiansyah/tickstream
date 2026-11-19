//! Metrik server dalam format Prometheus, dibaca pemilik lewat `GET /metrics`.
//!
//! Semua nama metrik ada di file ini. Bagian lain server cuma memanggil fungsi pencatat di
//! bawah, jadi daftar lengkap apa yang diukur (dan label apa saja yang dipakai) terbaca di
//! satu tempat. Label nggak pernah berisi simbol, IP, token, atau API key.
//!
//! Crate `metrics` memisahkan *mencatat* dari *menyimpan*: makro `counter!`/`gauge!` mengirim
//! ke "recorder" yang sedang terpasang. Di production itu recorder Prometheus (dipasang
//! `install`); di test, recorder lokal per test.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, http::header, response::IntoResponse};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

use crate::{auth::Admin, state::AppState};

const START_TIME: &str = "tickstream_start_time_seconds";
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

/// `GET /metrics`: semua metrik dalam format teks Prometheus. Khusus pemilik.
pub async fn render(_: Admin, State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
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
