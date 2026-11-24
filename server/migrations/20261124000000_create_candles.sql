CREATE TABLE candles (
    symbol        TEXT             NOT NULL,
    minute        TIMESTAMPTZ      NOT NULL,
    open          DOUBLE PRECISION NOT NULL,
    high          DOUBLE PRECISION NOT NULL,
    low           DOUBLE PRECISION NOT NULL,
    close         DOUBLE PRECISION NOT NULL,
    volume        DOUBLE PRECISION NOT NULL,
    -- Decides which close wins when a late trade arrives in a later flush.
    last_trade_ms BIGINT           NOT NULL,
    PRIMARY KEY (symbol, minute)
);
