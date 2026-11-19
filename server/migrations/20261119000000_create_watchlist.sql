-- Satu watchlist default (belum ada user/auth). Urutan tampil = urutan ditambahkan (id).
CREATE TABLE watchlist (
    id       BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- CHECK menyamai normalize_symbol di server: DB tetap konsisten walau ada yang
    -- menulis tanpa lewat API.
    symbol   TEXT NOT NULL UNIQUE CHECK (symbol ~ '^[A-Z0-9.:-]{1,20}$'),
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Isi awal = daftar yang dulu di-hardcode di web/src/App.tsx.
INSERT INTO watchlist (symbol) VALUES
    ('AAPL'), ('NVDA'), ('MSFT'), ('TSLA'), ('AMZN'),
    ('GOOGL'), ('META'), ('SPY'), ('BINANCE:BTCUSDT');
