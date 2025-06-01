.PHONY: all build test clippy clippy-fix format format-check clean doc run help

# Default target
all: format clippy test build

# Build the project
build:
	cargo build --all-features

# Build release version
release:
	cargo build --release --all-features

# Run tests
test:
	cargo test --all-features

# Run clippy with the same settings as GitHub Actions
CLIPPY_FLAGS = -D clippy::all \
               -D clippy::pedantic \
               -D clippy::nursery \
               -D clippy::cargo \
               -A clippy::module_name_repetitions \
               -A clippy::must_use_candidate \
               -A clippy::missing_errors_doc \
               -A clippy::missing_panics_doc \
               -A clippy::missing_docs_in_private_items \
               -A clippy::missing_const_for_fn

CLIPPY_EXAMPLE_FLAGS = $(CLIPPY_FLAGS) \
                       -A clippy::uninlined_format_args \
                       -A clippy::map_unwrap_or \
                       -A clippy::manual_let_else \
                       -A clippy::needless_collect \
                       -A clippy::single_match_else \
                       -A clippy::option_if_let_else

# Run clippy exactly as CI does
clippy:
	@echo "Running clippy on main code..."
	cargo clippy -- $(CLIPPY_FLAGS)
	@echo "\nRunning clippy on tests..."
	cargo clippy --tests -- $(CLIPPY_FLAGS)
	@echo "\nRunning clippy on examples..."
	cargo clippy --examples -- $(CLIPPY_EXAMPLE_FLAGS)

# Run basic clippy (less strict, for quick checks during development)
clippy-quick:
	@echo "Running basic clippy checks..."
	cargo clippy --all-targets --all-features -- -D warnings

# Run clippy and automatically fix what it can
clippy-fix:
	cargo clippy --fix --allow-dirty -- $(CLIPPY_FLAGS)
	cargo clippy --tests --fix --allow-dirty -- $(CLIPPY_FLAGS)
	cargo clippy --examples --fix --allow-dirty -- $(CLIPPY_EXAMPLE_FLAGS)

# Run rustfmt
format:
	cargo fmt --all

# Check formatting without changing files
format-check:
	cargo fmt --all -- --check

# Clean build artifacts
clean:
	cargo clean

# Build documentation
doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Run the server
run:
	cd spec-server && cargo run

# Run specific example
run-example:
	@echo "Usage: make run-example EXAMPLE=basic_workflow"
	@echo "Available examples:"
	@echo "  - basic_workflow"
	@echo "  - projection_example"
ifdef EXAMPLE
	cargo run --example $(EXAMPLE)
endif

# Development workflow - format, lint, test, build
dev: format clippy test build

# CI simulation - run all checks as CI would
ci: format-check clippy test doc build

# Show help
help:
	@echo "Available targets:"
	@echo "  make all          - Format, lint, test, and build (default)"
	@echo "  make build        - Build the project"
	@echo "  make release      - Build release version"
	@echo "  make test         - Run tests"
	@echo "  make clippy       - Run clippy with CI settings"
	@echo "  make clippy-quick - Run basic clippy (less strict)"
	@echo "  make clippy-fix   - Run clippy and auto-fix issues"
	@echo "  make format       - Format code with rustfmt"
	@echo "  make format-check - Check formatting without changes"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make doc          - Build documentation"
	@echo "  make run          - Run the server"
	@echo "  make run-example  - Run an example (use EXAMPLE=name)"
	@echo "  make dev          - Development workflow"
	@echo "  make ci           - Simulate CI checks locally"
	@echo "  make help         - Show this help message"