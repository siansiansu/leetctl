# Development shortcuts. `make` on its own prints the list.
#
# Targets carrying a `##` comment show up in `make help`; ones without stay hidden.

CARGO ?= cargo
PYTHON ?= python3

.DEFAULT_GOAL := help
.PHONY: help build release install run test test-pym test-ci lint fmt fmt-check check sets doc clean

help: ## Print this list
	@printf '\033[1mleetctl\033[0m — make targets\n\n'
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-11s\033[0m %s\n", $$1, $$2}'
	@printf '\nPass arguments to a run with ARGS, e.g. `make run ARGS="review --all"`.\n'

## --- build ---

build: ## Debug build
	$(CARGO) build

release: ## Optimised build
	$(CARGO) build --release

install: ## Install the binary into ~/.cargo/bin
	$(CARGO) install --path .

run: ## Run the debug binary, e.g. `make run ARGS="review next"`
	$(CARGO) run -- $(ARGS)

## --- test ---

test: ## Run the test suite
	$(CARGO) test

# Needs a Python dev install on the loader path; without one the test binary aborts
# before any test runs, which is why it is not part of `make check`.
test-pym: ## Run the suite including the optional pyo3 `--plan` path
	$(CARGO) test --features pym

test-ci: ## Run the suite exactly as CI does (needs cargo-nextest)
	$(CARGO) nextest run --release --all-features

## --- quality ---

lint: ## Clippy over every target, warnings denied as in CI
	$(CARGO) clippy --all-targets --all-features -- -D warnings

fmt: ## Format the tree
	$(CARGO) fmt

fmt-check: ## Fail if anything is unformatted, as in CI
	$(CARGO) fmt --check

check: fmt-check lint test ## Everything CI gates on, in one go

## --- data & docs ---

sets: ## Regenerate data/sets/*.toml from the curated sources
	$(PYTHON) scripts/gen_sets.py

doc: ## Build the API docs and open them
	$(CARGO) doc --no-deps --open

## --- housekeeping ---

clean: ## Remove the build directory
	$(CARGO) clean
