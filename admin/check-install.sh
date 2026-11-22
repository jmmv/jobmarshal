#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
make_cmd=$1
prefix=$2
bindir=$3
docdir=$4
man1dir=$5
libexecdir=$6
wrappers=$7
systemd=$8
systemduserunitdir=$9
shift 9
systemduserenvironmentgeneratordir=$1
profileddir=$2
stage=$(mktemp -d)
export XDG_DATA_HOME="$stage/data"
cleanup() {
    if test -x "$stage$bindir/jobmarshal"; then
        JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stop >/dev/null 2>&1 || :
    fi
    rm -rf "$stage"
}
trap cleanup EXIT HUP INT TERM
install_make() {
    "$make_cmd" "$@" DESTDIR="$stage" prefix="$prefix" bindir="$bindir" docdir="$docdir" \
        man1dir="$man1dir" libexecdir="$libexecdir" systemduserunitdir="$systemduserunitdir" \
        systemduserenvironmentgeneratordir="$systemduserenvironmentgeneratordir" profileddir="$profileddir"
}
install_make install
test -x "$stage$bindir/jobmarshal"
test -f "$stage$man1dir/jobmarshal.1"
test -x "$stage$libexecdir/jobmarshal-wrapper"
test -f "$stage$man1dir/jobmarshal-wrapper.1"
if test "$systemd" = yes; then
    test -f "$stage$systemduserunitdir/jobmarshal.service"
    test -x "$stage$systemduserenvironmentgeneratordir/60-jobmarshal"
    test -f "$stage$profileddir/jobmarshal.sh"
    grep -F ' serve' "$stage$systemduserunitdir/jobmarshal.service" > /dev/null
fi
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" print-env --format=systemd > "$stage/stdout"
test ! -e "$stage/runtime"
for name in $wrappers; do
    test ! -L "$stage$bindir/$name"
    test "$stage$bindir/$name" -ef "$stage$libexecdir/jobmarshal-wrapper"
done
# Reinstall checks ownership before the wrapper inode changes.
install_make install
for name in $wrappers; do
    test "$stage$bindir/$name" -ef "$stage$libexecdir/jobmarshal-wrapper"
done
"$stage$bindir/jobmarshal" help > "$stage/stdout" 2> "$stage/stderr"
grep -F "Jobmarshal home page: https://github.com/jmmv/jobmarshal" "$stage/stdout" >/dev/null
test ! -s "$stage/stderr"
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" start -j 2 \
    > "$stage/stdout" 2> "$stage/stderr"
test ! -s "$stage/stderr"
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" status > "$stage/stdout"
grep -F 'jobs=2 active_builds=0 waiting_builds=0 draining=false' "$stage/stdout" >/dev/null
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" JOBMARSHAL_JOBS=2 \
    CARGO_MAKEFLAGS='' MAKEFLAGS='' MFLAGS='' \
    PATH="$stage$bindir:$PATH" "$stage$libexecdir/jobmarshal-wrapper" sh -c 'exit 0'
check_run() {
    if test "$stage$bindir/../libexec/jobmarshal-wrapper" -ef \
        "$stage$libexecdir/jobmarshal-wrapper"; then
        run_layout=yes
        JOBMARSHAL_RUNTIME_DIR="$stage/runtime" JOBMARSHAL_JOBS=2 \
            CARGO_MAKEFLAGS='' MAKEFLAGS='' MFLAGS='' \
            PATH="$stage$bindir:$PATH" "$stage$bindir/jobmarshal" run -- sh -c 'exit 0'
    else
        run_layout=no
        if JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" run sh -c 'exit 0' \
            > "$stage/stdout" 2> "$stage/stderr"; then
            echo 'Run unexpectedly accepted a broken relative installation layout' >&2
            exit 1
        else
            test "$?" -eq 1
        fi
        grep -F 'cannot execute wrapper' "$stage/stderr" > /dev/null
    fi
}
check_run
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stats > "$stage/stdout"
grep -F 'Max jobs              2' "$stage/stdout" >/dev/null
grep -F 'Token counts are a snapshot;' "$stage/stdout" >/dev/null
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stats --zero > "$stage/stdout"
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stop
test ! -e "$stage/runtime/control.sock"
rm -rf "$stage/runtime"
rm "$stage/stdout" "$stage/stderr"
relocated=$(mktemp -d)
rmdir "$relocated"
mv "$stage" "$relocated"
stage=$relocated
export XDG_DATA_HOME="$stage/data"
check_run
if test "$run_layout" = yes; then
    JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stop
fi
rm -rf "$stage/runtime"
rm -f "$stage/stdout" "$stage/stderr"
for file in LICENSE NEWS.md README.md; do
    test -f "$stage$docdir/$file"
done
install_make uninstall
if test -n "$(find "$stage" -type f -print)"; then
    echo 'Uninstall left installed files behind' >&2
    exit 1
fi
