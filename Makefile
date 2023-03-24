.DEFAULT_GOAL := cargo_check

.PHONY: cargo_check
cargo_check:
	cargo check
