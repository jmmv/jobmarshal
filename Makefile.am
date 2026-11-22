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
	admin/check-markers.sh admin/check-wrappers.sh .github/workflows/test.yml .github/workflows/audit.yml

AM_DISTCHECK_CONFIGURE_FLAGS = --enable-wrappers='@JOBMARSHAL_WRAPPERS@'

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
	sh "$(abs_top_srcdir)/admin/check-wrappers.sh" "$(MAKE)" "$(abs_top_builddir)/$(PACKAGE)-$(VERSION).tar.gz" "$(cargo_target_dir)"
lint:
	cd "$(abs_top_srcdir)" && $(cargo_env) prek run --all-files
clean-local:
	cd "$(abs_top_srcdir)" && $(cargo_env) $(CARGO) clean $(cargo_manifest)
.PHONY: install-wrapper-preflight
# Check ownership while the previous wrapper inode still identifies its links.
install-dist_docDATA install-man1 install-exec-local: install-wrapper-preflight
install-wrapper-preflight:
	@set -eu; \
	bindir="$(DESTDIR)$(bindir)"; wrapper="$(DESTDIR)$(libexecdir)/jobmarshal-wrapper"; \
	if test -L "$$wrapper" || { test -e "$$wrapper" && test ! -f "$$wrapper"; }; then \
	    echo "Refusing to replace unrelated wrapper path: $$wrapper" >&2; exit 1; \
	fi; \
	for name in @JOBMARSHAL_WRAPPERS@; do \
	    destination="$$bindir/$$name"; \
	    if test -e "$$destination" || test -L "$$destination"; then \
	        if test -L "$$destination" || test ! -f "$$destination" || test ! "$$destination" -ef "$$wrapper"; then \
	            echo "Refusing to replace unrelated wrapper destination: $$destination" >&2; exit 1; \
	        fi; \
	    fi; \
	done; \
	$(MKDIR_P) "$$bindir" "$(DESTDIR)$(libexecdir)"; \
	if test -n '@JOBMARSHAL_WRAPPERS@'; then \
	    libwork=$$(mktemp -d "$(DESTDIR)$(libexecdir)/.jobmarshal-install.XXXXXXXX"); \
	    bindwork=; \
	    trap 'rm -f "$$libwork/probe"; rmdir "$$libwork"; if test -n "$$bindwork"; then rm -f "$$bindwork/probe"; rmdir "$$bindwork"; fi' EXIT HUP INT TERM; \
	    bindwork=$$(mktemp -d "$$bindir/.jobmarshal-links.XXXXXXXX"); \
	    : > "$$libwork/probe"; \
	    if ! ln "$$libwork/probe" "$$bindwork/probe"; then \
	        echo 'Cannot install wrapper hardlinks: bindir and libexecdir must support hardlinks on the same filesystem' >&2; exit 1; \
	    fi; \
	fi
install-exec-local: release
	@set -eu; \
	work=$$(mktemp -d "$(DESTDIR)$(libexecdir)/.jobmarshal-install.XXXXXXXX"); \
	trap 'rm -f "$$work/wrapper"; rmdir "$$work"' EXIT HUP INT TERM; \
	$(INSTALL_PROGRAM) "$(cargo_target_dir)/release/jobmarshal-wrapper" "$$work/wrapper"; \
	$(INSTALL_PROGRAM) "$(cargo_target_dir)/release/jobmarshal" "$(DESTDIR)$(bindir)/jobmarshal"; \
	for name in @JOBMARSHAL_WRAPPERS@; do rm -f "$(DESTDIR)$(bindir)/$$name"; done; \
	mv -f "$$work/wrapper" "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper"; \
	for name in @JOBMARSHAL_WRAPPERS@; do \
	    if ! ln "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper" "$(DESTDIR)$(bindir)/$$name"; then \
	        echo "Cannot install wrapper hardlink $$name; check bindir/libexecdir permissions and filesystem" >&2; exit 1; \
	    fi; \
	done
uninstall-local:
	@set -eu; \
	for name in @JOBMARSHAL_WRAPPERS@; do \
	    destination="$(DESTDIR)$(bindir)/$$name"; \
	    if test ! -L "$$destination" && test -f "$$destination" && \
	        test "$$destination" -ef "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper"; then \
	        rm -f "$$destination"; \
	    fi; \
	done; \
	rm -f "$(DESTDIR)$(libexecdir)/jobmarshal-wrapper" "$(DESTDIR)$(bindir)/jobmarshal"
installcheck-local:
	sh "$(abs_top_srcdir)/admin/check-install.sh" "$(MAKE)" "$(prefix)" "$(bindir)" "$(docdir)" \
	    "$(man1dir)" "$(libexecdir)" '@JOBMARSHAL_WRAPPERS@'
