# git-dragon GitHub Action runtime image.
#
# Stage 1 compiles the release binary. TLS is pure Rust (ureq rustls backend
# with embedded webpki-roots), so no system OpenSSL is needed in either stage.
# Stage 2 ships only the binary, git, and certificates needed by the
# entrypoint to refresh profile assets on the output branch.

FROM rust:1-slim-bookworm AS build

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/target/release/git-dragon /usr/local/bin/git-dragon
COPY action/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
