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

AC_CONFIG_FILES([Makefile])
AC_OUTPUT
