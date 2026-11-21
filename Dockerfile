FROM rust:latest AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
RUN mkdir -p /app/data

COPY --from=builder /usr/src/app/target/release/ferro_kv /app/server

ENV SERVER_ADDR=0.0.0.0:6379
ENV AOF_PATH=/app/data/appendonly.aof

EXPOSE 6379

CMD ["/app/server"]