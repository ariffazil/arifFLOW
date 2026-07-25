# arifFlow Makefile — governed Rust execution engine
# DITEMPA BUKAN DIBERI

.PHONY: all build test check clean release bench fmt lint prove

all: build test

build:
	cargo build --release

test:
	cargo test

check:
	cargo check

clean:
	cargo clean

release: build
	@echo "✅ arifFlow release binary at target/release/ariflow"

bench:
	cargo bench 2>/dev/null || @echo "Benchmarks: add [[bench]] section to Cargo.toml"

fmt:
	cargo fmt -- --check

lint:
	cargo clippy -- -D warnings

prove: test lint fmt
	@echo "✅ arifFlow PROVE: all gates passed"

sot-check:
	@echo "SOT: $(shell git rev-parse HEAD)"
	@echo "Tests: $(shell cargo test 2>&1 | tail -1)"
	@echo "Status: $$(git status -s | wc -l) dirty files"
