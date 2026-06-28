.PHONY: all release run clean bundle-darwin

all: release

release:
	cargo build --release

run:
	cargo run

# ── macOS .app bundle ──────────────────────────
bundle-darwin: release
	./scripts/build-dmg.sh

dmg: release
	./scripts/build-dmg.sh

# ── Cross-compilation helpers ──────────────────
build-darwin-arm64:
	cargo build --release --target aarch64-apple-darwin

build-darwin-x64:
	cargo build --release --target x86_64-apple-darwin

build-windows-x64:
	cargo build --release --target x86_64-pc-windows-msvc

clean:
	cargo clean
	rm -rf target/release/bundle-darwin
