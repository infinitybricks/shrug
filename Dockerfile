FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p shrug-ai

FROM debian:stable-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/shrug /usr/local/bin/shrug
EXPOSE 8080
ENV BIND_ADDR=0.0.0.0:8080
CMD ["shrug"]
