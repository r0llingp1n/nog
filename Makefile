# nog Makefile

.PHONY: all build check check-versions clean fmt fmt-check help init-pre-commit-hooks install lint pre-commit-run-all-files reinstall release run test uninstall

# Variables
BIN              := nog
PREFIX           ?= $(HOME)
BINDIR           ?= $(PREFIX)/bin
CARGO_TARGET_DIR ?= target
RELEASE_BIN      := $(CARGO_TARGET_DIR)/release/$(BIN)
DEBUG_BIN        := $(CARGO_TARGET_DIR)/debug/$(BIN)

.DEFAULT_GOAL := build

all: build ## Build the debug binary

build: ## Build the debug binary
	cargo build

release: ## Build the optimized release binary
	cargo build --release

run: ## Run nog against the current repo
	cargo run

install: release ## Build release and install the binary (see Binaries below)
	mkdir -p $(DESTDIR)$(BINDIR)
	install -m 755 $(RELEASE_BIN) $(DESTDIR)$(BINDIR)/$(BIN)
	@echo "Installed $(BIN) -> $(DESTDIR)$(BINDIR)/$(BIN)"
	@case ":$$PATH:" in \
		*":$(BINDIR):"*) ;; \
		*) echo "Warning: $(BINDIR) is not on your PATH";; \
	esac

uninstall: ## Remove the installed binary
	rm -f $(DESTDIR)$(BINDIR)/$(BIN)

reinstall: uninstall install ## Uninstall then install

test: ## Run the test suite
	cargo test

check: ## Type-check without producing binaries
	cargo check

fmt: ## Format sources in place
	cargo fmt

fmt-check: ## Fail if sources are not formatted
	cargo fmt --check

lint: ## Run clippy with warnings denied
	cargo clippy --all-targets -- -D warnings

check-versions: ## Verify rust-toolchain.toml and .tool-versions agree
	./scripts/check-toolchain-versions.sh

init-pre-commit-hooks: ## Install the pre-commit git hook
	@command -v pre-commit >/dev/null 2>&1 || { \
		echo "pre-commit not found. Install it with 'pipx install pre-commit'"; \
		echo "or see https://pre-commit.com/#installation"; \
		exit 1; }
	pre-commit install
	@echo "Hook installed. It runs on every 'git commit'."

pre-commit-run-all-files: ## Run every pre-commit hook against all files
	pre-commit run --all-files

clean: ## Remove build artifacts
	cargo clean

help: ## Show this help message
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Binaries:"
	@printf "  \033[36m%-20s\033[0m %s\n" "debug" "$(DEBUG_BIN)"
	@printf "  \033[36m%-20s\033[0m %s\n" "release" "$(RELEASE_BIN)"
	@printf "  \033[36m%-20s\033[0m %s\n" "installed" "$(BINDIR)/$(BIN)"
	@echo
	@echo "Override the install location with PREFIX or BINDIR:"
	@echo "  make install BINDIR=/usr/local/bin"
