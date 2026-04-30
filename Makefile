.PHONY: run build validator test-all test clippy fmt

SOUNDFONT ?= assets/soundfonts/FluidR3_GM.sf2
SCORE ?= ../after-the-void/main.ly
DEFAULT_RUN_ARGS = --soundfont $(SOUNDFONT) --score $(SCORE)
RUN_ARGS = $(if $(ARGS),$(ARGS),$(filter-out run,$(MAKECMDGOALS)))

run: validator
	cargo run -- $(if $(RUN_ARGS),$(RUN_ARGS),$(DEFAULT_RUN_ARGS))

build: validator
	cargo build

validator:
	cargo build -p lilypalooza-plugin-validator --bin lilypalooza-plugin-validator

test-all:
	cargo test --workspace --all-features --all-targets

test: test-all

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	cargo +nightly fmt --all

ifeq ($(firstword $(MAKECMDGOALS)),run)
.PHONY: $(RUN_ARGS)
$(RUN_ARGS):
	@:
endif
