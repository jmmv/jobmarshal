#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

# The builtin precheck keeps disabled users from launching any programs.  Let
# systemctl resolve masks and precedence only when an enablement link exists.
jobmarshal_enabled() {
    case ${XDG_CONFIG_HOME-} in
        /*) set -- "$XDG_CONFIG_HOME/systemd/user" ;;
        *) set -- "${HOME-}/.config/systemd/user" ;;
    esac
    case ${XDG_RUNTIME_DIR-} in
        /*) set -- "$@" "$XDG_RUNTIME_DIR/systemd/user" ;;
    esac
    set -- "$@" /etc/systemd/user /run/systemd/user
    while test "$#" -gt 0; do
        if test -L "$1/default.target.wants/jobmarshal.service"; then
            case $("@systemctl@" --user --root=/ is-enabled jobmarshal.service 2>/dev/null) in
                enabled|enabled-runtime) return 0 ;;
                *) return 1 ;;
            esac
        fi
        shift
    done
    return 1
}
