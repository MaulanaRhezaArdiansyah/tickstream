# Satu image berisi server Rust + halaman React hasil build.
# Build dari root repo: docker build -t tickstream .

# ---- 1. Halaman web
FROM node:24-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ---- 2. Server. SQLX_OFFLINE: cek query pakai cache server/.sqlx, tanpa database.
FROM rust:1-bookworm AS server
WORKDIR /server
ENV SQLX_OFFLINE=true
COPY server/ ./
# Cache registry dan target antar build, supaya build ulang nggak mulai dari nol.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/server/target \
    cargo build --release --locked && cp target/release/server /usr/local/bin/server

# ---- 3. Image akhir: cuma binary, halaman, dan sertifikat CA untuk HTTPS ke Finnhub.
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home tickstream
COPY --from=server /usr/local/bin/server /usr/local/bin/server
COPY --from=web /web/dist /srv/web
USER tickstream
ENV BIND_ADDR=0.0.0.0:8080 \
    WEB_DIR=/srv/web \
    RUST_LOG=info
EXPOSE 8080
CMD ["server"]
