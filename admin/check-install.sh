#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
make_cmd=$1
prefix=$2
bindir=$3
docdir=$4
man1dir=$5
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
"$make_cmd" install DESTDIR="$stage" prefix="$prefix" bindir="$bindir" docdir="$docdir" man1dir="$man1dir"
test -x "$stage$bindir/jobmarshal"
test -f "$stage$man1dir/jobmarshal.1"
"$stage$bindir/jobmarshal" help > "$stage/stdout" 2> "$stage/stderr"
grep -F "Jobmarshal home page: https://github.com/jmmv/jobmarshal" "$stage/stdout" >/dev/null
test ! -s "$stage/stderr"
rm "$stage/stdout" "$stage/stderr"
for file in LICENSE NEWS.md README.md; do
    test -f "$stage$docdir/$file"
done
"$make_cmd" uninstall DESTDIR="$stage" prefix="$prefix" bindir="$bindir" docdir="$docdir" man1dir="$man1dir"
if test -n "$(find "$stage" -type f -print)"; then
    echo 'Uninstall left installed files behind' >&2
    exit 1
fi
