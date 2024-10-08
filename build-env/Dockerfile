ARG BASE_IMAGE=rust:1.80.1-slim-bullseye
WORKDIR /home/project

FROM $BASE_IMAGE as planner

# Add i686 target
RUN rustup target add i686-unknown-linux-gnu

# Add Musl target
RUN rustup target add i686-unknown-linux-musl
RUN rustup target add x86_64-unknown-linux-musl

# Add RaspberryPi target
RUN rustup target add armv7-unknown-linux-gnueabihf
RUN rustup target add aarch64-unknown-linux-gnu

# Cross compile for Windows
RUN rustup target add x86_64-pc-windows-gnu

# Helper to make deb packages.
RUN cargo install cargo-deb
### use with
# cargo deb --target=x86_64-unknown-linux-gnu

# Tp-Note needs some libs for crosscompilation.
RUN dpkg --add-architecture i386; apt update

RUN apt-get -y install pkg-config apt-utils \
        # Pack artifacts in archive.
        # Some dependencies need Python, `mc` is for convenience.
        zip python3 mc \
        # for `cargo build --target i686-unknown-linux-gnu`
        libc6-dev-i386 build-essential crossbuild-essential-i386 \
        # for `cargo deb --target=i686-unknown-linux-gnu`
        libgcc1:i386 \
        # for `cargo build --target armv7-unknown-linux-gnueabihf`
        musl-tools crossbuild-essential-armhf \
        # for `cargo build --target aarch64-unknown-linux-gnu`
        crossbuild-essential-arm64 \
        # for `cargo build --release --target x86_64-pc-windows-gnu`
        binutils-mingw-w64 mingw-w64
                      


COPY . .

