# SPDX-License-Identifier: GPL-3.0-only
AUTOMAKE_OPTIONS = no-dependencies

dist_doc_DATA = LICENSE NEWS.md README.md
rust_sources = jobmarshal/src/main.rs
rust_tests =
rust_manifests = Cargo.toml jobmarshal/Cargo.toml
EXTRA_DIST = $(rust_sources) $(rust_tests) $(rust_manifests) Cargo.lock \
	rust-toolchain.toml rustfmt.toml prek.toml .gitignore \
	admin/check-dependencies.sh admin/check-install.sh admin/check-sources.sh \
	admin/check-markers.sh .github/workflows/test.yml .github/workflows/audit.yml

cargo_target_dir = $(abs_top_builddir)/target
cargo_env = CARGO_TARGET_DIR="$(cargo_target_dir)"
cargo_manifest = --manifest-path="$(abs_top_srcdir)/Cargo.toml"

.PHONY: debug release lint
all-local: release
# One recipe owns the release build, even with parallel make.
release:
	$(cargo_env) $(CARGO) build $(cargo_manifest) --workspace --locked --release
debug:
	$(cargo_env) $(CARGO) build $(cargo_manifest) --workspace --locked
check-local: debug
	$(cargo_env) $(CARGO) test $(cargo_manifest) --workspace --locked
	cd "$(abs_top_srcdir)" && sh admin/check-sources.sh
lint:
	cd "$(abs_top_srcdir)" && $(cargo_env) prek run --all-files
clean-local:
	$(cargo_env) $(CARGO) clean $(cargo_manifest)
install-exec-local: release
	$(MKDIR_P) "$(DESTDIR)$(bindir)"
	$(INSTALL_PROGRAM) "$(cargo_target_dir)/release/jobmarshal" "$(DESTDIR)$(bindir)/jobmarshal"
uninstall-local:
	rm -f "$(DESTDIR)$(bindir)/jobmarshal"
installcheck-local:
	sh "$(abs_top_srcdir)/admin/check-install.sh" "$(MAKE)" "$(prefix)" "$(bindir)" "$(docdir)"
