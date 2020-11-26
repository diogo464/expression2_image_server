FROM rust:1.48.0 as builder

RUN mkdir expression2_image_server
WORKDIR expression2_image_server
RUN mkdir src
ADD src/ src/
COPY ./Cargo.toml ./

RUN rustup default nightly
RUN cargo build --release

FROM fedora:33

EXPOSE 8080

RUN useradd -m server
WORKDIR /home/server
COPY --from=builder /expression2_image_server/target/release/expression2_image_server e2server
COPY images images
RUN chown -R server:server .
USER server

ENTRYPOINT ["./e2server"]
