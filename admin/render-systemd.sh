#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
# Paths travel through the environment so awk does not interpret their escapes.
JOBMARSHAL_EXECUTABLE=$4 SYSTEMCTL_EXECUTABLE=$5 JOBMARSHAL_GUARD=$2 \
    awk '
function quote(value, service,    result, i, c) {
    result = service ? "\"" : "\047"
    for (i = 1; i <= length(value); i++) {
        c = substr(value, i, 1)
        if (service) {
            if (c == "\\" || c == "\"") result = result "\\"
            if (c == "%") result = result c
            result = result c
        } else {
            result = result (c == "\047" ? "\047\\\047\047" : c)
        }
    }
    return result (service ? "\"" : "\047")
}
function replace(line, token, value,    position, result) {
    result = ""
    while ((position = index(line, token))) {
        result = result substr(line, 1, position - 1) value
        line = substr(line, position + length(token))
    }
    return result line
}
function render(line) {
    line = replace(line, "@jobmarshal@", quote(ENVIRON["JOBMARSHAL_EXECUTABLE"], service))
    # Guard templates already have quotes around this placeholder.
    line = replace(line, "\"@systemctl@\"", quote(ENVIRON["SYSTEMCTL_EXECUTABLE"], 0))
    print line
}
BEGIN { service = ARGV[1] ~ /\.service\.in$/ }
/^@enablement@$/ {
    while ((getline guard < ENVIRON["JOBMARSHAL_GUARD"]) > 0) render(guard)
    close(ENVIRON["JOBMARSHAL_GUARD"])
    next
}
{ render($0) }
' "$1" > "$3.tmp"
mv -f "$3.tmp" "$3"
