#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
cd "$(dirname "$0")/.."
matches=$(find . -type d \( -name .git -o -name .jj -o -name .tickets -o -name target -o -name build -o -name autom4te.cache \) -prune -o \
    -type f ! -name configure ! -name config.log ! -name Makefile ! -name '*.tar.gz' \
    -exec grep -nH '[D]O NOT SUBMIT' {} + || :)
if test -n "$matches"; then
    echo "$matches" >&2
    exit 1
fi
