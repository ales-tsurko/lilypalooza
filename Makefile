.PHONY: run build validator check test-all code-health lint fmt fmt-vendor-check

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
CRAP_EXCLUDES := --exclude '**/build.rs' --exclude '**/benches/**' --exclude '**/examples/**' --exclude '**/tests.rs' --exclude '**/*_tests.rs' --exclude 'crates/vendor/**'
SIMILARITY_PATHS := \
	src \
	crates/editor-host/src \
	crates/lilypalooza-audio/src \
	crates/lilypalooza-builtins/src \
	crates/lilypalooza-clap/src \
	crates/lilypalooza-egui-baseview/src \
	crates/lilypalooza-plugin-scan/src \
	crates/lilypalooza-plugin-validator/src \
	crates/lilypalooza-vst3/src
SIMILARITY_ARGS := $(SIMILARITY_PATHS) --threshold 0.92 --min-lines 12 --min-tokens 80 --fail-on-duplicates
VENDOR_MANIFESTS := \
	crates/vendor/iced-code-editor/Cargo.toml \
	crates/vendor/iced_aw/Cargo.toml \
	crates/vendor/tree-sitter-lilypond/Cargo.toml
VENDOR_RUSTFMT_CONFIG := rustfmt.vendor.toml

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

code-health:
	similarity-rs $(SIMILARITY_ARGS)
	cargo machete --skip-target-dir
	cargo llvm-cov $(CARGO_PACKAGES) --all-features --all-targets --lcov --output-path $(COVERAGE_LCOV)
	cargo crap --workspace --lcov $(COVERAGE_LCOV) $(CRAP_EXCLUDES) --format markdown --output $(CRAP_REPORT)
	cargo crap --workspace --lcov $(COVERAGE_LCOV) $(CRAP_EXCLUDES) --summary --fail-above
	@set -e; \
	for package in $(MIRI_PACKAGES); do \
		MIRIFLAGS="$(MIRI_FLAGS)" cargo +nightly miri test -p "$$package" --all-features; \
	done

lint:
	@status=0; \
	cargo clippy $(CARGO_PACKAGES) --all-targets --all-features -- -D warnings || status=$$?; \
	cargo +nightly fmt --check $(CARGO_PACKAGES) || status=$$?; \
	exit $$status

fmt:
	cargo +nightly fmt $(CARGO_PACKAGES)

fmt-vendor-check:
	@set -e; \
	for manifest in $(VENDOR_MANIFESTS); do \
		dir=$$(dirname "$$manifest"); \
		config="$(VENDOR_RUSTFMT_CONFIG)"; \
		if [ -f "$$dir/rustfmt.toml" ]; then config="$$dir/rustfmt.toml"; fi; \
		if [ -f "$$dir/.rustfmt.toml" ]; then config="$$dir/.rustfmt.toml"; fi; \
		cargo fmt --manifest-path "$$manifest" --check -- --config-path "$$config"; \
	done

ifeq ($(firstword $(MAKECMDGOALS)),run)
.PHONY: $(RUN_ARGS)
$(RUN_ARGS):
	@:
endif
