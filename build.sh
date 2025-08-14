cargo build --release --target x86_64-pc-windows-gnu
cargo build --release --target x86_64-unknown-linux-gnu
mkdir out
mv target/x86_64-pc-windows-gnu/release/xps-19-proxy.exe out/XPS-Proxy-win-x64.exe
mv target/x86_64-unknown-linux-gnu/release/xps-19-proxy out/XPS-Proxy-linux-x64
