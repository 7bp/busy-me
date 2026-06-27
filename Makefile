.PHONY: all release run clean bundle-darwin

all: release

release:
	cargo build --release

run:
	cargo run

# ── macOS .app bundle ──────────────────────────
bundle-darwin: release
	mkdir -p target/release/bundle-darwin/Busy\ Me.app/Contents/MacOS
	mkdir -p target/release/bundle-darwin/Busy\ Me.app/Contents/Resources
	cp macos/Info.plist target/release/bundle-darwin/Busy\ Me.app/Contents/Info.plist
	cp target/release/busy-me target/release/bundle-darwin/Busy\ Me.app/Contents/MacOS/busy-me
	# Generate .icns from a 1024x1024 PNG if available, otherwise skip
	-ls Resources/icon.icns 2>/dev/null && cp Resources/icon.icns target/release/bundle-darwin/Busy\ Me.app/Contents/Resources/ || true
	@echo "---"
	@echo "Bundle created at: target/release/bundle-darwin/Busy Me.app"
	@echo "Run: open target/release/bundle-darwin/Busy\\ Me.app"

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
