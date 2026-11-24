FROM docker.io/library/rust:1.100.0-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN set -eu; \
    host="$(rustc -vV | sed -n 's/^host: //p')"; \
    target="${host%-gnu}-musl"; \
    rustup target add "$target"; \
    RUSTFLAGS="-C linker=rust-lld" cargo build --locked --release --target "$target"; \
    cp "target/$target/release/media-organizer" /media-organizer

FROM scratch
COPY --from=build /media-organizer /usr/local/bin/media-organizer
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/media-organizer"]
CMD ["--version"]
