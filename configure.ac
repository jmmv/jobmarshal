dnl SPDX-License-Identifier: GPL-3.0-only
AC_PREREQ([2.69])
AC_INIT([jobmarshal], [0.1.0], [https://github.com/jmmv/jobmarshal/issues],
        [jobmarshal], [https://github.com/jmmv/jobmarshal])
dnl Keep explicit yes/no basenames distinct from boolean option spellings.
jobmarshal_wrapper_option=
for jobmarshal_argument; do
    AS_CASE([$jobmarshal_argument],
        [--enable-wrappers], [jobmarshal_wrapper_option=missing],
        [--enable-wrappers=*], [jobmarshal_wrapper_option=value],
        [--disable-wrappers], [jobmarshal_wrapper_option=disabled])
done
AC_CONFIG_AUX_DIR([admin])
AC_CONFIG_SRCDIR([Cargo.toml])
AM_INIT_AUTOMAKE([1.16 foreign no-dependencies])
AC_PATH_PROG([CARGO], [cargo])
AS_IF([test -z "$CARGO"], [AC_MSG_ERROR([cargo is required])])

AC_ARG_ENABLE([wrappers],
    [AS_HELP_STRING([--enable-wrappers='cargo cc mold'],
        [install hardlinks for the selected command basenames (default: none)])],
    [jobmarshal_wrapper_selection=$enableval], [jobmarshal_wrapper_selection=])
AS_CASE([$jobmarshal_wrapper_option],
    [missing], [AC_MSG_ERROR([--enable-wrappers requires a list of command basenames])],
    [disabled], [jobmarshal_wrapper_selection=])
jobmarshal_wrapper_names=
dnl Disable pathname expansion while splitting the untrusted whitespace-separated list.
AS_CASE([$-], [*f*], [jobmarshal_restore_glob=no], [jobmarshal_restore_glob=yes; set -f])
for jobmarshal_name in $jobmarshal_wrapper_selection; do
    AS_CASE([$jobmarshal_name],
        [[[!A-Za-z0-9]]*|*[[!A-Za-z0-9_.+-]]*|jobmarshal|jobmarshal-wrapper],
        [AC_MSG_ERROR([invalid wrapper basename: $jobmarshal_name])])
    jobmarshal_wrapper_names="$jobmarshal_wrapper_names $jobmarshal_name"
done
AS_IF([test "$jobmarshal_restore_glob" = yes], [set +f])
JOBMARSHAL_WRAPPERS=$(
    printf '%s\n' $jobmarshal_wrapper_names | LC_ALL=C sort -u | tr '\n' ' ' | sed 's/ $//'
)
AC_SUBST([JOBMARSHAL_WRAPPERS])
jobmarshal_wrapper_summary=$JOBMARSHAL_WRAPPERS
AS_IF([test -z "$jobmarshal_wrapper_summary"], [jobmarshal_wrapper_summary=none])
AC_MSG_NOTICE([selected wrappers: $jobmarshal_wrapper_summary])

AC_ARG_WITH([systemd],
    [AS_HELP_STRING([--with-systemd@<:@=auto|yes|no@:>@],
        [install systemd user integration (default: auto)])], [], [with_systemd=auto])
AS_CASE([$with_systemd], [auto|yes|no], [],
    [AC_MSG_ERROR([--with-systemd accepts auto, yes, or no])])
AC_ARG_VAR([systemduserunitdir], [systemd vendor user unit directory])
AC_ARG_VAR([systemduserenvironmentgeneratordir], [systemd user environment generator directory])
AC_ARG_VAR([profileddir], [login shell profile directory (default: /etc/profile.d)])
AC_ARG_VAR([PKG_CONFIG], [path to pkg-config])
AC_ARG_VAR([SYSTEMD_PATH], [path to systemd-path])
AC_ARG_VAR([SYSTEMCTL], [path to systemctl])
AS_IF([test -z "$profileddir"], [profileddir=/etc/profile.d])
AS_IF([test "$with_systemd" != no], [
    AC_PATH_PROG([PKG_CONFIG], [pkg-config])
    AC_PATH_PROG([SYSTEMD_PATH], [systemd-path])
    AC_PATH_PROG([SYSTEMCTL], [systemctl])
    AS_IF([test -z "$systemduserunitdir" && test -n "$PKG_CONFIG"], [
        systemduserunitdir=$("$PKG_CONFIG" --variable=systemduserunitdir systemd 2>/dev/null)
    ])
    AS_IF([test -z "$systemduserunitdir" && test -n "$SYSTEMD_PATH"], [
        systemduserunitdir=$("$SYSTEMD_PATH" systemd-user-unit 2>/dev/null)
    ])
    AS_IF([test -z "$systemduserenvironmentgeneratordir" && test -n "$systemduserunitdir"], [
        systemduserenvironmentgeneratordir="${systemduserunitdir%/*}/user-environment-generators"
    ])
])
jobmarshal_systemd=no
AS_IF([test "$with_systemd" != no && test -n "$SYSTEMCTL" &&
       test -n "$systemduserunitdir" && test -n "$systemduserenvironmentgeneratordir"], [
    jobmarshal_systemd=yes
    for jobmarshal_directory in "$systemduserunitdir" "$systemduserenvironmentgeneratordir" "$profileddir"; do
        AS_CASE([$jobmarshal_directory], [/*], [],
            [AC_MSG_ERROR([systemd installation directories must be absolute])])
    done
], [
    AS_IF([test "$with_systemd" = yes], [
        AC_MSG_ERROR([systemd integration requires systemctl and a user unit directory; install systemd tooling or set SYSTEMCTL and systemduserunitdir])
    ])
])
AM_CONDITIONAL([WITH_SYSTEMD], [test "$jobmarshal_systemd" = yes])
AC_SUBST([jobmarshal_systemd])
AC_MSG_NOTICE([systemd integration: $jobmarshal_systemd])
AS_IF([test "$jobmarshal_systemd" = yes], [
    AC_MSG_NOTICE([systemd user units: $systemduserunitdir])
    AC_MSG_NOTICE([systemd environment generators: $systemduserenvironmentgeneratordir])
    AC_MSG_NOTICE([shell profiles: $profileddir])
])

AC_CONFIG_FILES([Makefile])
AC_OUTPUT
