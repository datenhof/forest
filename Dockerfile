FROM --platform=$BUILDPLATFORM rust:1.85-bookworm as chef
WORKDIR /app
ENV PKG_CONFIG_SYSROOT_DIR=/
# RUN apk add --no-cache musl-dev openssl-dev zig gcompat clang19-libclang
RUN apt-get update && apt-get install -y pkg-config gcc-multilib libssl-dev libclang-dev
# Install ZIG
RUN curl -L https://ziglang.org/builds/zig-linux-x86_64-0.14.0-dev.3367+1cc388d52.tar.xz | tar -xJ --strip-components=1 -C /usr/local/bin
RUN cargo install --locked cargo-zigbuild cargo-chef
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
RUN mkdir /app/linux
# Install CFSSL
RUN curl -L https://github.com/cloudflare/cfssl/releases/download/v1.6.5/cfssl_1.6.5_linux_amd64 -o /app/linux/amd64_cfssl && \
    curl -L https://github.com/cloudflare/cfssl/releases/download/v1.6.5/cfssl_1.6.5_linux_arm64 -o /app/linux/arm64_cfssl && \
    chmod +x /app/linux/amd64_cfssl && chmod +x /app/linux/arm64_cfssl

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json --release --zigbuild \
    --target x86_64-unknown-linux-gnu --target aarch64-unknown-linux-gnu
 
COPY . .
RUN cargo zigbuild -r --target x86_64-unknown-linux-gnu --target aarch64-unknown-linux-gnu && \
    cp target/aarch64-unknown-linux-gnu/release/forest /app/linux/arm64 && \
    cp target/x86_64-unknown-linux-gnu/release/forest /app/linux/amd64

# Second stage putting the build result into a debian slim image
FROM debian:bookworm-slim AS runtime
ARG TARGETPLATFORM
COPY --from=builder /app/${TARGETPLATFORM} /app/forest
COPY --from=builder /app/${TARGETPLATFORM}_cfssl /app/cfssl
ENTRYPOINT [ "/app/forest" ]
CMD [ "help" ]