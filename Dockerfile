# syntax=docker/dockerfile:1.3
FROM rust:1.70.0-alpine AS build

WORKDIR /usr/local/app

COPY . .

RUN \
    --mount=type=cache,target=/var/cache/apk \
    apk add build-base && \
    cargo build --release

FROM gcr.io/distroless/static-debian11:latest

COPY --from=build /usr/local/app/target/release/stringsext /stringsext

ENTRYPOINT ["/stringsext"]
