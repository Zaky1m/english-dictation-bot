# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
# Build the dependencies first so source edits do not recompile them.
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/deps/eng* target/release/eng*
COPY src ./src
COPY migrations ./migrations
COPY texts ./texts
COPY web/i18n ./web/i18n
COPY build.rs ./
RUN cargo build --release --locked

FROM debian:bookworm-slim
# ffmpeg is only used by the audio generation commands and the admin tools.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /app eng
WORKDIR /app
COPY --from=build /src/target/release/eng /usr/local/bin/eng
COPY web ./web
RUN mkdir -p data && chown -R eng:eng /app
USER eng
# PORT is overridden by hosting platforms that assign their own.
ENV PORT=4510 \
    DATA_DIR=/app/data \
    WEB_DIR=/app/web
EXPOSE 4510
CMD ["eng", "serve"]
