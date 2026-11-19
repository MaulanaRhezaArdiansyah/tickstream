//! Berita perusahaan dari Finnhub `/company-news`, dirapikan sebelum dikirim ke browser:
//! link yang bukan http(s) dan judul kosong dibuang, berita 24 jam terakhir dihitung, lalu
//! diurutkan dari yang terbaru dan dipotong ke `MAX_ITEMS`.

use serde::{Deserialize, Deserializer, Serialize};

/// Panel cuma menampilkan segini; sisanya nggak dikirim.
const MAX_ITEMS: usize = 20;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Satu artikel apa adanya dari Finnhub. Field lain (`image`, `category`, `related`)
/// diabaikan. Finnhub kadang mengirim `null` atau nggak mengirim field sama sekali, dan satu
/// artikel yang nggak lengkap nggak boleh menggagalkan seluruh daftar.
#[derive(Debug, Deserialize)]
pub struct NewsArticle {
    #[serde(default, deserialize_with = "null_as_default")]
    pub id: i64,
    /// Unix timestamp dalam detik.
    #[serde(default, deserialize_with = "null_as_default")]
    pub datetime: i64,
    #[serde(default, deserialize_with = "null_as_default")]
    pub headline: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub source: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub summary: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub url: String,
}

/// `null` diperlakukan sama dengan field yang nggak dikirim: nilai kosong (`""` / `0`).
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Artikel yang dikirim ke browser.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewsItem {
    pub id: i64,
    /// Unix timestamp dalam milidetik, sama dengan trade.
    pub datetime: i64,
    pub headline: String,
    pub source: String,
    pub summary: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompanyNews {
    /// Jumlah berita 24 jam terakhir, dihitung SEBELUM dipotong, supaya saham yang ramai
    /// nggak semuanya tampil "20".
    #[serde(rename = "count24h")]
    pub count_24h: usize,
    /// Terbaru di depan, paling banyak `MAX_ITEMS`.
    pub items: Vec<NewsItem>,
}

pub fn prepare_news(articles: Vec<NewsArticle>, now_ms: i64) -> CompanyNews {
    let mut items: Vec<NewsItem> = articles
        .into_iter()
        // Link lain (mis. `javascript:`) bisa menjalankan script waktu diklik di browser.
        .filter(|a| a.url.starts_with("https://") || a.url.starts_with("http://"))
        .filter(|a| !a.headline.trim().is_empty())
        .map(|a| NewsItem {
            id: a.id,
            // Angka ngawur dari luar nggak boleh bikin server panic.
            datetime: a.datetime.saturating_mul(1000),
            headline: a.headline,
            source: a.source,
            summary: a.summary,
            url: a.url,
        })
        .collect();
    let count_24h = items
        .iter()
        .filter(|item| item.datetime >= now_ms - DAY_MS)
        .count();
    items.sort_by_key(|item| std::cmp::Reverse(item.datetime));
    items.truncate(MAX_ITEMS);
    CompanyNews { count_24h, items }
}

/// Tanggal UTC `YYYY-MM-DD` dari Unix timestamp (detik), format yang diminta Finnhub.
/// Algoritma `civil_from_days` Howard Hinnant, supaya nggak perlu crate tanggal.
pub fn iso_date(unix_secs: i64) -> String {
    let z = unix_secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR_MS: i64 = 3_600_000;

    fn article(id: i64, datetime: i64, headline: &str, url: &str) -> NewsArticle {
        NewsArticle {
            id,
            datetime,
            headline: headline.to_string(),
            source: "Reuters".to_string(),
            summary: String::new(),
            url: url.to_string(),
        }
    }

    fn ok(id: i64, datetime: i64) -> NewsArticle {
        article(id, datetime, "Judul", "https://example.com")
    }

    #[test]
    fn newest_first_and_capped_at_20() {
        let news = prepare_news((1..=25).map(|i| ok(i, i)).collect(), 0);
        assert_eq!(news.items.len(), 20);
        assert_eq!(news.items[0].datetime, 25_000);
        assert_eq!(news.items[19].datetime, 6_000);
    }

    #[test]
    fn count_24h_is_taken_before_the_cap() {
        let now_ms = 100_000_000;
        let mut articles: Vec<_> = (0..30).map(|i| ok(i, (now_ms - HOUR_MS) / 1000)).collect();
        articles.push(ok(30, (now_ms - 25 * HOUR_MS) / 1000));
        articles.push(ok(31, (now_ms - 25 * HOUR_MS) / 1000));
        let news = prepare_news(articles, now_ms);
        assert_eq!(news.count_24h, 30);
        assert_eq!(news.items.len(), 20);
    }

    #[test]
    fn exactly_24h_old_counts() {
        let now_ms = 100_000_000;
        let news = prepare_news(vec![ok(1, (now_ms - 24 * HOUR_MS) / 1000)], now_ms);
        assert_eq!(news.count_24h, 1);
    }

    #[test]
    fn unsafe_links_and_blank_headlines_are_dropped() {
        let news = prepare_news(
            vec![
                article(1, 1, "Script", "javascript:alert(1)"),
                article(2, 2, "Ftp", "ftp://x"),
                article(3, 3, "Kosong", ""),
                article(4, 4, "   ", "https://blank"),
                article(5, 5, "Aman", "https://a"),
                article(6, 6, "Aman juga", "http://b"),
            ],
            0,
        );
        let urls: Vec<_> = news.items.iter().map(|item| item.url.as_str()).collect();
        assert_eq!(urls, ["http://b", "https://a"]);
    }

    #[test]
    fn missing_fields_default_to_empty() {
        let articles: Vec<NewsArticle> = serde_json::from_str(
            r#"[{"id":1,"datetime":10,"headline":"H","url":"https://x","summary":null,"image":""}]"#,
        )
        .unwrap();
        let news = prepare_news(articles, 0);
        assert_eq!(news.items[0].source, "");
        assert_eq!(news.items[0].summary, "");
    }

    #[test]
    fn null_numbers_default_to_zero() {
        let articles: Vec<NewsArticle> = serde_json::from_str(
            r#"[{"id":null,"datetime":null,"headline":"H","url":"https://x"}]"#,
        )
        .unwrap();
        let news = prepare_news(articles, 0);
        assert_eq!(news.items[0].id, 0);
        assert_eq!(news.items[0].datetime, 0);
    }

    #[test]
    fn absurd_datetime_does_not_overflow() {
        let news = prepare_news(vec![ok(1, i64::MAX)], 0);
        assert_eq!(news.items[0].datetime, i64::MAX);
    }

    #[test]
    fn iso_date_formats_utc_days() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(1_709_164_800), "2024-02-29");
        assert_eq!(iso_date(1_795_132_800), "2026-11-20");
        assert_eq!(iso_date(1_795_132_799), "2026-11-19");
    }
}
