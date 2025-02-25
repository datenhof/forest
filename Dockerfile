FROM rust:1.85-bookworm as builder

RUN echo "deb [trusted=yes] https://apt.fury.io/compscidr/ /" > /etc/apt/sources.list.d/zig.list
RUN apt-get update && apt-get install -y \
    libssl-dev \
    zig
RUN cargo install --locked cargo-zigbuild cargo-chef
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu

# First build a dummy project with our dependencies to cache them in Docker
WORKDIR /usr/src
RUN cargo new --bin forest
WORKDIR /usr/src/forest
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo zigbuild --release --target x86_64-unknown-linux-gnu --target aarch64-unknown-linux-gnu
RUN rm src/*.rs
RUN mkdir -p /app/linux

# Now copy the sources and do the real build
COPY . .
RUN cargo test
RUN cargo zigbuild --release --target x86_64-unknown-linux-gnu --target aarch64-unknown-linux-gnu
RUN cp target/x86_64-unknown-linux-gnu/release/forest /app/linux/amd64 && \
    cp target/aarch64-unknown-linux-gnu/release/forest /app/linux/arm64

# Second stage putting the build result into a debian jessie-slim image
FROM debian:bookworm-slim
ARG TARGETPLATFORM
COPY --from=builder /app/${TARGETPLATFORM} /usr/local/bin/forest
CMD forest