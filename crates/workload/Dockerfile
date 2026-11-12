FROM rust:1.99.1-slim-trixie@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p workload

FROM debian:bullseye-20230612
COPY --from=build /src/target/release/workload /workload
USER 65532:65532
EXPOSE 8080 9090
STOPSIGNAL SIGINT
HEALTHCHECK NONE
ENTRYPOINT ["/workload"]
