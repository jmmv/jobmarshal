#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
cd "$(dirname "$0")/.."
list=$(mktemp)
trap 'rm -f "$list"' EXIT HUP INT TERM
# All handwritten build inputs must appear in Automake distribution lists.
awk '{ for (i = 1; i <= NF; i++) if ($i != "\\") print $i }' Makefile.am | sed '/^$/d' | sort -u > "$list"
find . -type d \( -name .git -o -name .jj -o -name .tickets -o -name target -o -name build -o -name autom4te.cache \) -prune -o \
    -type f \( -name '*.rs' -o -name Cargo.toml -o -name Cargo.lock -o -name '*.sh' -o -path './systemd/*.in' -o -name '*.yml' -o -name '*.toml' -o -name '*.[1-9]' \) \
    ! -name install-sh ! -name missing ! -path ./jobmarshal.sh -print | sed 's,^./,,' | sort | while IFS= read -r file; do
    if ! grep -Fqx "$file" "$list"; then
        echo "Missing distribution input: $file" >&2
        exit 1
    fi
done
