#!/sh/bin

rm -rf ppdrive_test
cargo build --release

mkdir ppdrive_test
cp target/release/server ppdrive_test/
cp target/release/ppdrive ppdrive_test/
