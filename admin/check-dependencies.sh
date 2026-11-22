#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
cargo metadata --locked --no-deps --format-version 1 >/dev/null
cargo machete
if cargo tree --locked --prefix none | grep -Eq '^openssl(-sys)? v'; then
    echo 'OpenSSL dependencies are not allowed' >&2
    exit 1
fi
