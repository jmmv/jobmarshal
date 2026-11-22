# SPDX-License-Identifier: GPL-3.0-only
AUTOMAKE_OPTIONS = no-dependencies

dist_doc_DATA = LICENSE NEWS.md README.md
dist_man1_MANS = man/jobmarshal.1 man/jobmarshal-wrapper.1
rust_sources = jobmarshal/src/main.rs jobmarshal/src/lib.rs jobmarshal/src/cli.rs \
	jobmarshal/src/daemon.rs jobmarshal/src/lifecycle.rs common/src/lib.rs \
	common/src/capacity.rs common/src/client.rs common/src/locks.rs \
	common/src/protocol.rs common/src/runtime.rs common/src/descriptors.rs \
	jobmarshal/src/admission.rs jobmarshal/src/pool.rs wrapper/src/main.rs \
	wrapper/src/lib.rs wrapper/src/admission.rs wrapper/src/execution.rs \
	wrapper/src/inherited.rs wrapper/src/invocation.rs
rust_tests = jobmarshal/tests/integration_test.rs jobmarshal/tests/common/mod.rs \
	jobmarshal/tests/suites/mod.rs jobmarshal/tests/suites/cli_suite.rs \
	jobmarshal/tests/common/service.rs jobmarshal/tests/suites/lifecycle_suite.rs \
	tests/common/mod.rs wrapper/tests/integration_test.rs wrapper/tests/common/mod.rs \
	wrapper/tests/suites/mod.rs wrapper/tests/suites/admission_suite.rs \
	wrapper/tests/suites/cargo_suite.rs wrapper/tests/suites/execution_suite.rs \
	wrapper/tests/suites/native_suite.rs wrapper/tests/fixtures/native.rs \
	wrapper/tests/fixtures/build.rs
rust_manifests = Cargo.toml common/Cargo.toml jobmarshal/Cargo.toml wrapper/Cargo.toml
EXTRA_DIST = $(rust_sources) $(rust_tests) $(rust_manifests) Cargo.lock \
	rust-toolchain.toml rustfmt.toml prek.toml .gitignore .cargo/audit.toml \
	admin/check-dependencies.sh admin/check-install.sh admin/check-sources.sh \
	admin/check-markers.sh .github/workflows/test.yml .github/workflows/audit.yml

cargo_target_dir = $(abs_top_builddir)/target
cargo_env = CARGO_TARGET_DIR="$(cargo_target_dir)"
cargo_manifest = --manifest-path="$(abs_top_srcdir)/Cargo.toml"

.PHONY: debug release lint
all-local: release
# One recipe owns the release build, even with parallel make.
release:
	cd "$(abs_top_srcdir)" && $(cargo_env) $(CARGO) build $(cargo_manifest) --workspace --locked --release
debug:
	cd "$(abs_top_srcdir)" && $(cargo_env) $(CARGO) build $(cargo_manifest) --workspace --locked
# Cross-binary integration tests require the workspace binaries built above.
check-local: debug
	cd "$(abs_top_srcdir)" && $(cargo_env) $(CARGO) test $(cargo_manifest) --workspace --locked
	cd "$(abs_top_srcdir)" && sh admin/check-sources.sh
lint:
	cd "$(abs_top_srcdir)" && $(cargo_env) prek run --all-files
clean-local:
	cd "$(abs_top_srcdir)" && $(cargo_env) $(CARGO) clean $(cargo_manifest)
install-exec-local: release
	$(MKDIR_P) "$(DESTDIR)$(bindir)" "$(DESTDIR)$(libexecdir)"
	$(INSTALL_PROGRAM) "$(cargo_target_dir)/release/jobmarshal" "$(DESTDIR)$(bindir)/jobmarshal"
	$(INSTALL_PROGRAM) "$(cargo_target_dir)/release/jobmarshal-wrapper" \
	    "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper"
uninstall-local:
	rm -f "$(DESTDIR)$(bindir)/jobmarshal"
	rm -f "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper"
installcheck-local:
	sh "$(abs_top_srcdir)/admin/check-install.sh" "$(MAKE)" "$(prefix)" "$(bindir)" "$(docdir)" "$(man1dir)" "$(libexecdir)"
