FROM ekidd/rust-musl-builder as builder

RUN mkdir expression2_image_server
WORKDIR expression2_image_server
RUN mkdir src
ADD src/ src/
COPY ./Cargo.toml ./

RUN cargo build --release

FROM alpine:latest

EXPOSE 8080

RUN apk update \
    && apk add --no-cache ca-certificates tzdata \
    && rm -rf /var/cache/apk/*

RUN adduser -D server
WORKDIR /home/server
COPY --from=builder /home/rust/src/expression2_image_server/target/x86_64-unknown-linux-musl/release/expression2_image_server e2server
COPY images images
RUN chown -R server:server .
USER server

ENTRYPOINT ["./e2server"]
