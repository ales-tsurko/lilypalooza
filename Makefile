.PHONY: run build validator check test-all test test-cov miri lint clippy fmt

SOUNDFONT ?= assets/soundfonts/FluidR3_GM.sf2
SCORE ?= ../after-the-void/main.ly
DEFAULT_RUN_ARGS = --soundfont $(SOUNDFONT) --score $(SCORE)
RUN_ARGS = $(if $(ARGS),$(ARGS),$(filter-out run,$(MAKECMDGOALS)))
CARGO_PACKAGES := \
	-p lilypalooza \
	-p editor-host \
	-p lilypalooza-audio \
	-p lilypalooza-builtins \
	-p lilypalooza-clap \
	-p lilypalooza-egui-baseview \
	-p lilypalooza-plugin-scan \
	-p lilypalooza-plugin-validator \
	-p lilypalooza-vst3
MIRI_FLAGS := -Zmiri-disable-isolation -Zmiri-tree-borrows
MIRI_PACKAGES := lilypalooza-clap lilypalooza-plugin-scan lilypalooza-plugin-validator
COVERAGE_LCOV := target/lilypalooza-lcov.info
CRAP_REPORT := target/lilypalooza-crap.md
CRAP_EXCLUDES := --exclude '**/build.rs' --exclude 'crates/vendor/**'

run: validator
	cargo run -- $(if $(RUN_ARGS),$(RUN_ARGS),$(DEFAULT_RUN_ARGS))

build: validator
	cargo build

validator:
	cargo build -p lilypalooza-plugin-validator --bin lilypalooza-plugin-validator

check:
	cargo check $(CARGO_PACKAGES) --all-features --all-targets

test-all:
	cargo test $(CARGO_PACKAGES) --all-features --all-targets
	$(MAKE) miri

test: test-all

test-cov:
	cargo llvm-cov $(CARGO_PACKAGES) --all-features --all-targets --lcov --output-path $(COVERAGE_LCOV)
	cargo crap --workspace --lcov $(COVERAGE_LCOV) $(CRAP_EXCLUDES) --format markdown --output $(CRAP_REPORT)
	cargo crap --workspace --lcov $(COVERAGE_LCOV) $(CRAP_EXCLUDES) --summary --fail-above

miri:
	@set -e; for package in $(MIRI_PACKAGES); do MIRIFLAGS="$(MIRI_FLAGS)" cargo +nightly miri test -p "$$package" --all-features; done

lint:
	cargo clippy $(CARGO_PACKAGES) --all-targets --all-features -- -D warnings
	cargo +nightly fmt --check $(CARGO_PACKAGES)

clippy:
	cargo clippy $(CARGO_PACKAGES) --all-targets --all-features -- -D warnings

fmt:
	cargo +nightly fmt $(CARGO_PACKAGES)

ifeq ($(firstword $(MAKECMDGOALS)),run)
.PHONY: $(RUN_ARGS)
$(RUN_ARGS):
	@:
endif
