PROJECT := hev-socks5-tunnel
RUSTDIR := rust
BINDIR := bin
PROFILE ?= release
CARGO ?= cargo

ifeq ($(PROFILE),release)
	CARGO_PROFILE_FLAG := --release
	RUST_TARGET_DIR := $(RUSTDIR)/target/release
else
	CARGO_PROFILE_FLAG :=
	RUST_TARGET_DIR := $(RUSTDIR)/target/$(PROFILE)
endif

EXEC_SOURCE := $(RUST_TARGET_DIR)/hs5t
EXEC_TARGET := $(BINDIR)/$(PROJECT)

.PHONY: all exec test fmt clean static shared install uninstall

all: exec

exec:
	$(CARGO) build --manifest-path $(RUSTDIR)/Cargo.toml -p hs5t-bin $(CARGO_PROFILE_FLAG)
	mkdir -p $(BINDIR)
	cp $(EXEC_SOURCE) $(EXEC_TARGET)

test:
	$(CARGO) test --manifest-path $(RUSTDIR)/Cargo.toml --workspace

fmt:
	$(CARGO) fmt --manifest-path $(RUSTDIR)/Cargo.toml --all

clean:
	rm -rf $(BINDIR)
	$(CARGO) clean --manifest-path $(RUSTDIR)/Cargo.toml

static shared install uninstall:
	@echo "error: target '$@' is not implemented for the Rust migration yet" >&2
	@exit 1
