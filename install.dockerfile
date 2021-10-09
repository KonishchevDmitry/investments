FROM rust as build
ARG CACHE_DATE
RUN --mount=type=cache,id=investments-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=investments-cargo-target,target=/var/tmp/cargo-target \
    CARGO_TARGET_DIR=/var/tmp/cargo-target cargo install investments

FROM debian:stable-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/cargo/bin/investments /usr/local/bin/investments
ENTRYPOINT ["investments"]