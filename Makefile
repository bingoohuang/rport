linux:
	cross build --release --target x86_64-unknown-linux-musl
build:
	cargo build --release
clean:
	cargo clean
install:
	cargo install --path .
test:
	cargo test
lint:
	cargo clippy
format:
	cargo fmt