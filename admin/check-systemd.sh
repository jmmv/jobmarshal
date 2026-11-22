#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
make_cmd=$1
archive=$2
target=$3
work=$(mktemp -d)
cleanup() {
    result=$?
    if test "$result" -ne 0; then
        for log in "$work/explicit/install.log" "$work/disabled/install.log"; do
            if test -f "$log"; then tail -60 "$log" >&2; fi
        done
    fi
    rm -rf "$work"
}
trap cleanup EXIT HUP INT TERM
"$make_cmd" dist > "$work/dist.log" 2>&1
mkdir "$work/source"
tar -xzf "$archive" -C "$work/source" --strip-components=1
source_dir=$work/source

configure_ok() {
    label=$1
    expected=$2
    shift 2
    mkdir "$work/$label"
    (cd "$work/$label" && "$source_dir/configure" "$@" > configure.log 2>&1)
    grep -Fqx "configure: systemd integration: $expected" "$work/$label/configure.log"
}
configure_bad() {
    label=$1
    shift
    mkdir "$work/$label"
    if (cd "$work/$label" && "$source_dir/configure" "$@" > configure.log 2>&1); then
        echo "Configure unexpectedly accepted: $*" >&2
        exit 1
    fi
    grep -F 'systemd' "$work/$label/configure.log" > /dev/null
}

cat > "$work/pkg-config" <<'TOOL'
#!/bin/sh
case $1:$2 in
    --variable=systemduserunitdir:systemd) echo /vendor/lib/systemd/user ;;
    *) exit 1 ;;
esac
TOOL
cat > "$work/systemd-path" <<'TOOL'
#!/bin/sh
case $1 in
    systemd-user-unit) echo /fallback/lib/systemd/user ;;
    *) exit 1 ;;
esac
TOOL
cat > "$work/systemctl" <<'TOOL'
#!/bin/sh
printf '%s\n' "$*" >> "$JOBMARSHAL_TEST_CALLS"
cat "$JOBMARSHAL_TEST_STATE"
TOOL
chmod +x "$work/pkg-config" "$work/systemd-path" "$work/systemctl"
configure_ok auto-missing no PKG_CONFIG=/bin/false SYSTEMD_PATH=/bin/false SYSTEMCTL=/bin/false
configure_bad required-missing --with-systemd PKG_CONFIG=/bin/false SYSTEMD_PATH=/bin/false SYSTEMCTL=/bin/false
configure_bad invalid --with-systemd=perhaps
configure_bad relative --with-systemd systemduserunitdir=relative SYSTEMCTL="$work/systemctl"
configure_ok disabled no --without-systemd PKG_CONFIG="$work/pkg-config" SYSTEMCTL="$work/systemctl"
configure_ok metadata yes PKG_CONFIG="$work/pkg-config" SYSTEMD_PATH=/bin/false SYSTEMCTL="$work/systemctl"
grep -Fqx 'systemduserunitdir = /vendor/lib/systemd/user' "$work/metadata/Makefile"
configure_ok fallback yes PKG_CONFIG=/bin/false SYSTEMD_PATH="$work/systemd-path" SYSTEMCTL="$work/systemctl"
grep -Fqx 'systemduserunitdir = /fallback/lib/systemd/user' "$work/fallback/Makefile"
configure_ok explicit yes --with-systemd systemduserunitdir=/custom/units \
    systemduserenvironmentgeneratordir=/custom/generators profileddir=/custom/profiles \
    PKG_CONFIG=/bin/false SYSTEMD_PATH=/bin/false SYSTEMCTL="$work/systemctl"

cd "$work/explicit"
prefix='/opt/jobmarshal with spaces'
run_make() {
    "$make_cmd" "$@" cargo_target_dir="$target" prefix="$prefix"
}
stage=$work/stage
run_make -j4 install DESTDIR="$stage" > install.log 2>&1
test -f "$stage/custom/units/jobmarshal.service"
test -x "$stage/custom/generators/60-jobmarshal"
test -f "$stage/custom/profiles/jobmarshal.sh"
grep -Fqx "ExecStart=\"$prefix/bin/jobmarshal\" serve" "$stage/custom/units/jobmarshal.service"
run_make uninstall DESTDIR="$stage" > uninstall.log 2>&1
test -z "$(find "$stage" ! -type d -print)"
(cd "$work/disabled" && run_make install DESTDIR="$stage" > install.log 2>&1)
test ! -e "$stage/vendor/lib/systemd/user/jobmarshal.service"
test ! -e "$stage/etc/profile.d/jobmarshal.sh"
JOBMARSHAL_RUNTIME_DIR="$work/not-created" XDG_DATA_HOME="$work/data" \
    "$stage$prefix/bin/jobmarshal" print-env --format=systemd > "$work/disabled-env"
test ! -e "$work/not-created"
(cd "$work/disabled" && run_make uninstall DESTDIR="$stage" > uninstall.log 2>&1)
test -z "$(find "$stage" ! -type d -print)"

# Exercise the installed shell templates against an isolated enablement layout.
export HOME="$work/home" XDG_CONFIG_HOME="$work/config" XDG_RUNTIME_DIR="$work/runtime"
export XDG_DATA_HOME="$work/data space '\"\$\`\\dir"
export JOBMARSHAL_RUNTIME_DIR="$work/daemon-runtime"
export JOBMARSHAL_TEST_CALLS="$work/calls" JOBMARSHAL_TEST_STATE="$work/state"
mkdir -p "$HOME" "$XDG_CONFIG_HOME/systemd/user/default.target.wants" \
    "$XDG_RUNTIME_DIR/systemd/user/default.target.wants" "$work/global/default.target.wants"
# Redirect global links too, without touching host configuration.
sed "s,/etc/systemd/user,$work/global,g; s,/run/systemd/user,$work/global-runtime,g" \
    "$source_dir/admin/systemd-enabled.sh" > "$work/guard"
render() {
    sh "$source_dir/admin/render-systemd.sh" "$source_dir/systemd/$1" "$work/guard" "$work/$2" "$3" "$work/systemctl"
}
quoted_executable="${XDG_DATA_HOME}-executable"
ln -s "$target/debug/jobmarshal" "$quoted_executable"
render jobmarshal.sh.in profile.sh "$quoted_executable"
render 60-jobmarshal.sh.in generator "$quoted_executable"
chmod +x "$work/generator"

# FreeBSD sh resets $? before sourcing a file; other shells expose the preceding
# status.  Compare sourcing behavior against a reference in the same shell.
cat > "$work/status-reference.sh" <<'PROFILE'
return "$?"
PROFILE
profile_status() {
    if sh -c '(exit 7); . "$1"' sh "$1"; then
        printf '0\n'
    else
        printf '%s\n' "$?"
    fi
}
expected_profile_status=$(profile_status "$work/status-reference.sh")
check_profile_status() {
    if test "$(profile_status "$1")" != "$expected_profile_status"; then
        echo 'Profile changed the status exposed by the sourcing shell' >&2
        exit 1
    fi
    # Execute the body after a failing command to test a nonzero entry status
    # even in shells that reset it when entering a sourced file.
    { printf '(exit 7)\n'; cat "$1"; } > "$work/status-body.sh"
    if sh "$work/status-body.sh"; then
        echo 'Profile lost its nonzero entry status' >&2
        exit 1
    else
        test "$?" -eq 7
    fi
}

check_disabled() {
    rm -f "$work/calls"
    "$work/generator" > "$work/stdout" 2> "$work/stderr"
    test ! -s "$work/stdout" && test ! -s "$work/stderr"
    sh -c '
        original_path=$PATH
        . "$1"
        test "$PATH" = "$original_path"
        test "${jobmarshal_profile_status+x}" != x
        test "${jobmarshal_profile_env+x}" != x
        ! command -v jobmarshal_enabled >/dev/null 2>&1
        ! command -v jobmarshal_profile_init >/dev/null 2>&1
    ' sh "$work/profile.sh"
    test ! -e "$work/calls"
    check_profile_status "$work/profile.sh"
}
check_disabled
# An unset or relative XDG_CONFIG_HOME uses HOME/.config.
XDG_CONFIG_HOME=relative check_disabled
mkdir -p "$HOME/.config/systemd/user/default.target.wants"
printf 'enabled\n' > "$work/state"
ln -s /unit "$HOME/.config/systemd/user/default.target.wants/jobmarshal.service"
XDG_CONFIG_HOME=relative "$work/generator" > "$work/env"
test -s "$work/env"
rm "$HOME/.config/systemd/user/default.target.wants/jobmarshal.service"

for location in "$XDG_CONFIG_HOME/systemd/user" "$XDG_RUNTIME_DIR/systemd/user" "$work/global"; do
    link=$location/default.target.wants/jobmarshal.service
    ln -s /unit "$link"
    for state in enabled enabled-runtime masked masked-runtime disabled not-found static indirect; do
        printf '%s\n' "$state" > "$work/state"
        "$work/generator" > "$work/env"
        case $state in
            enabled|enabled-runtime)
                test -s "$work/env"
                sh -c '
                    . "$1"
                    expected=$PATH
                    . "$1"
                    test "$PATH" = "$expected"
                    test "$JOBMARSHAL_RUNTIME_DIR" = "$2"
                    case $PATH in "$XDG_DATA_HOME/jobmarshal/bin":*) ;; *) exit 1 ;; esac
                    test "${jobmarshal_profile_status+x}" != x
                    ! command -v jobmarshal_enabled >/dev/null 2>&1
                    ! command -v jobmarshal_profile_init >/dev/null 2>&1
                ' sh "$work/profile.sh" "$JOBMARSHAL_RUNTIME_DIR"
                check_profile_status "$work/profile.sh"
                ;;
            *) test ! -s "$work/env" ;;
        esac
    done
    rm "$link"
done
test ! -e "$JOBMARSHAL_RUNTIME_DIR"
grep -Fqx -- '--user --root=/ is-enabled jobmarshal.service' "$work/calls"

# A missing executable must neither print diagnostics nor corrupt the shell.
printf 'enabled\n' > "$work/state"
ln -s /unit "$XDG_CONFIG_HOME/systemd/user/default.target.wants/jobmarshal.service"
render jobmarshal.sh.in missing-profile.sh "$work/missing executable"
sh -c '. "$1"; test "$?" -eq 0' sh "$work/missing-profile.sh" > "$work/stdout" 2> "$work/stderr"
test ! -s "$work/stdout" && test ! -s "$work/stderr"
check_profile_status "$work/missing-profile.sh"
cat > "$work/failing-command" <<'TOOL'
#!/bin/sh
printf 'export JOBMARSHAL_TEST_UNEXPECTED=yes\n'
exit 1
TOOL
chmod +x "$work/failing-command"
render jobmarshal.sh.in failing-profile.sh "$work/failing-command"
sh -c '. "$1"; test "${JOBMARSHAL_TEST_UNEXPECTED+x}" != x' sh "$work/failing-profile.sh"
check_profile_status "$work/failing-profile.sh"

# Real offline systemctl must respect masks without connecting to the user manager.
if command -v systemctl > /dev/null 2>&1; then
    cp "$source_dir/systemd/jobmarshal.service.in" "$XDG_CONFIG_HOME/systemd/user/jobmarshal.service"
    rm "$XDG_CONFIG_HOME/systemd/user/default.target.wants/jobmarshal.service"
    ln -s ../jobmarshal.service "$XDG_CONFIG_HOME/systemd/user/default.target.wants/jobmarshal.service"
    test "$(systemctl --user --root=/ is-enabled jobmarshal.service)" = enabled
    rm "$XDG_CONFIG_HOME/systemd/user/jobmarshal.service"
    ln -s /dev/null "$XDG_CONFIG_HOME/systemd/user/jobmarshal.service"
    test "$(systemctl --user --root=/ is-enabled jobmarshal.service || :)" = masked
    rm "$XDG_CONFIG_HOME/systemd/user/jobmarshal.service" "$XDG_CONFIG_HOME/systemd/user/default.target.wants/jobmarshal.service"
fi

# Verify the generated unit using real executable paths, without loading any units.
if command -v systemd-analyze > /dev/null 2>&1; then
    service_executable="$work/executable space \$%jobmarshal"
    ln -s "$target/debug/jobmarshal" "$service_executable"
    render jobmarshal.service.in jobmarshal.service "$service_executable"
    systemd-analyze --user --man=no verify "$work/jobmarshal.service"
fi
