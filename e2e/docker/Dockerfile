FROM rust:1.50.0
RUN apt update && apt install -y protobuf-compiler
RUN rustup component add rustfmt
COPY . /tmp
WORKDIR tmp/e2e
