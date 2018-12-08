rustup default stable
rustup target add i686-pc-windows-gnu
rustup target add x86_64-pc-windows-gnu

rustup default stable-i686-pc-windows-gnu
rustup set default-host i686-pc-windows-gnu
cargo build --target i686-pc-windows-gnu       --release

rustup default stable-x86_64-pc-windows-gnu
rustup set default-host x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu     --release
