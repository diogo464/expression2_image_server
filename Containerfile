FROM docker.io/rust:1.69.0 as builder

RUN mkdir expression2_image_server
WORKDIR expression2_image_server
RUN mkdir src
ADD src/ src/
COPY ./Cargo.toml ./

RUN rustup default nightly
RUN cargo build --release

FROM docker.io/debian:buster-slim

EXPOSE 8080
LABEL org.opencontainers.image.source https://github.com/diogo464/expression2-image-server

RUN apt update && apt install -y openssl ca-certificates && apt clean
RUN useradd -m server
WORKDIR /home/server
COPY --from=builder /expression2_image_server/target/release/expression2_image_server e2server
COPY images images
RUN chown -R server:server .
USER server

ENTRYPOINT ["./e2server"]
