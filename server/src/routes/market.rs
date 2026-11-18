use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{error::AppError, market::Quote, state::AppState};

#[derive(Deserialize)]
pub struct SymbolQuery {
    symbol: String,
}

/// `GET /api/quote?symbol=NVDA`
pub async fn quote(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<Quote>, AppError> {
    let symbol = normalize_symbol(&params.symbol)?;
    let quote = state.market.quote(&symbol).await?;
    Ok(Json(quote))
}

/// `" nvda "` → `"NVDA"`. Ticker US: huruf/angka, boleh titik atau strip (mis. `BRK.B`), maks 10 karakter.
fn normalize_symbol(raw: &str) -> Result<String, AppError> {
    let symbol = raw.trim().to_ascii_uppercase();
    let valid = (1..=10).contains(&symbol.len())
        && symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');

    if valid {
        Ok(symbol)
    } else {
        Err(AppError::BadRequest(format!("simbol tidak valid: {raw:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_symbol;

    #[test]
    fn normalizes_case_and_whitespace() {
        assert_eq!(normalize_symbol(" nvda ").unwrap(), "NVDA");
        assert_eq!(normalize_symbol("brk.b").unwrap(), "BRK.B");
    }

    #[test]
    fn rejects_invalid_symbols() {
        for raw in ["", "   ", "AAPL;DROP", "ABCDEFGHIJK", "NVDA&token=x"] {
            assert!(normalize_symbol(raw).is_err(), "harusnya ditolak: {raw:?}");
        }
    }
}
