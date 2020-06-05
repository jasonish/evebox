### Speed Up Building

This might help...

```
RUSTFLAGS="-C link-arg=-fuse-ld=lld" cargo build
```

### Building for MUSL

cargo install cross
cross build --release --target=x86_64-unknown-linux-musl --features="no-assets"

### Buliding for Raspberry Pi 4

Rust target: armv7-unknown-linux-gnueabihf

```asm
cargo install cross
cross test --target armv7-unknown-linux-gnueabihf
```

### Building for Windows

cargo install cross
cross build --target x86_64-pc-windows-gnu

or..

rustup target add x86_64-pc-windows-gnu

.cargo/config:
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-gcc-ar"


### MacOS

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
export PATH=/root/.cargo/bin:$PATH
rustup target add x86_64-apple-darwin
CC=o64-clang cargo build --target=x86_64-apple-darwin


helper container:
https://github.com/multiarch/crossbuild
