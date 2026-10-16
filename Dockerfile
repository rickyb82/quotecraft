# QuoteCraft server for a framework: `quotecraft-cli serve` plus the web app, with WordCraft's
# CLI beside it for the DOCX and PDF.
#
#   docker build -t quotecraft .
#   docker run -d -p 7878:7878 -v quotecraft-data:/data -e QUOTECRAFT_TOKEN=change-me quotecraft
#
# The data (price list, customers, quotes), the configuration and the documents produced live
# in /data. MailCraft's URL and service token go in the preferences (Settings, or
# `docker exec quotecraft quotecraft-cli prefs mailcraft_url=http://mailcraft:7878 mailcraft_token=…`).

FROM rust:1.97-bookworm AS build
WORKDIR /src
RUN rustup target add wasm32-unknown-unknown \
 && curl -sSL https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xz -C /usr/local/bin trunk
COPY . .
RUN cargo build --release -p quotecraft-cli \
 && cd apps/quotecraft-web && trunk build --release

# WordCraft's CLI (the documents). Built from its repository: long, but once per image.
FROM rust:1.97-bookworm AS wordcraft
ARG WORDCRAFT_REPO=https://github.com/rickyb82/wordcraft
ARG WORDCRAFT_REV=main
RUN apt-get update && apt-get install -y --no-install-recommends git && rm -rf /var/lib/apt/lists/* \
 && git clone --depth 1 --branch "$WORDCRAFT_REV" "$WORDCRAFT_REPO" /wordcraft \
 && cd /wordcraft && cargo build --release -p wordcraft-cli \
 && cp target/release/wordcraft-cli /usr/local/bin/wordcraft-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --create-home --home-dir /data quotecraft
COPY --from=build /src/target/release/quotecraft-cli /usr/local/bin/quotecraft-cli
COPY --from=build /src/dist/web /app/web
COPY --from=wordcraft /usr/local/bin/wordcraft-cli /usr/local/bin/wordcraft-cli
USER quotecraft
VOLUME ["/data"]
EXPOSE 7878
ENV QUOTECRAFT_HOME=/data
ENV QUOTECRAFT_TOKEN=""
CMD ["sh", "-c", "exec quotecraft-cli serve --bind 0.0.0.0:7878 --static /app/web --home /data --token \"$QUOTECRAFT_TOKEN\" ${QUOTECRAFT_LANG:+--lang $QUOTECRAFT_LANG}"]
