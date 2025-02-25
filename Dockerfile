FROM rust:1.85-alpine as builder

RUN apk add --no-cache musl-dev openssl-dev zig
RUN cargo install --locked cargo-zigbuild cargo-chef
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

# First build a dummy project with our dependencies to cache them in Docker
WORKDIR /usr/src
RUN cargo new --bin forest
WORKDIR /usr/src/forest
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo zigbuild --release --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl
RUN rm src/*.rs

# Now copy the sources and do the real build
COPY . .
RUN cargo test
RUN cargo zigbuild --release --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl
RUN mkdir -p /app/linux && \
    cp target/x86_64-unknown-linux-musl/release/forest /app/linux/amd64 && \
    cp target/aarch64-unknown-linux-musl/release/forest /app/linux/arm64

# Second stage putting the build result into a debian jessie-slim image
FROM alpine:latest AS runtime
ARG TARGETPLATFORM
COPY --from=builder /app/${TARGETPLATFORM} /usr/local/bin/forest
CMD forest