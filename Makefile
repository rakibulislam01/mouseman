BINARY=mouseman
BUILD_DIR=target/release

.PHONY: build install clean run check

build:
	cargo build --release

install: build
	cp $(BUILD_DIR)/$(BINARY) /usr/local/bin/$(BINARY)
	@echo "✅ Installed to /usr/local/bin/$(BINARY)"
	@echo ""
	@echo "Now set up your config:"
	@echo "  mkdir -p ~/.config/mouseman"
	@echo "  cp config.yaml ~/.config/mouseman/config.yaml"

run:
	cargo run -- --config config.yaml

run-verbose:
	cargo run -- --config config.yaml --verbose

check:
	cargo check
	cargo clippy -- -D warnings

clean:
	cargo clean
