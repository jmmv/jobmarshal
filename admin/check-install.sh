#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
make_cmd=$1
prefix=$2
bindir=$3
docdir=$4
man1dir=$5
stage=$(mktemp -d)
cleanup() {
    if test -x "$stage$bindir/jobmarshal"; then
        JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stop >/dev/null 2>&1 || :
    fi
    rm -rf "$stage"
}
trap cleanup EXIT HUP INT TERM
"$make_cmd" install DESTDIR="$stage" prefix="$prefix" bindir="$bindir" docdir="$docdir" man1dir="$man1dir"
test -x "$stage$bindir/jobmarshal"
test -f "$stage$man1dir/jobmarshal.1"
"$stage$bindir/jobmarshal" help > "$stage/stdout" 2> "$stage/stderr"
grep -F "Jobmarshal home page: https://github.com/jmmv/jobmarshal" "$stage/stdout" >/dev/null
test ! -s "$stage/stderr"
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" start -j 2 \
    > "$stage/stdout" 2> "$stage/stderr"
test ! -s "$stage/stderr"
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" status > "$stage/stdout"
grep -F 'jobs=2 active_builds=0 waiting_builds=0 draining=false' "$stage/stdout" >/dev/null
JOBMARSHAL_RUNTIME_DIR="$stage/runtime" "$stage$bindir/jobmarshal" stop
test ! -e "$stage/runtime/control.sock"
rm -rf "$stage/runtime"
rm "$stage/stdout" "$stage/stderr"
for file in LICENSE NEWS.md README.md; do
    test -f "$stage$docdir/$file"
done
"$make_cmd" uninstall DESTDIR="$stage" prefix="$prefix" bindir="$bindir" docdir="$docdir" man1dir="$man1dir"
if test -n "$(find "$stage" -type f -print)"; then
    echo 'Uninstall left installed files behind' >&2
    exit 1
fi
