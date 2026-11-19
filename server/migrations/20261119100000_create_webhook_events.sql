-- Event mentah dari webhook Finnhub. Disimpan apa adanya (JSONB) supaya nggak ada yang
-- hilang walau bentuknya berubah; diolah belakangan kalau sudah dibutuhkan.
CREATE TABLE webhook_events (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    payload     JSONB NOT NULL
);
