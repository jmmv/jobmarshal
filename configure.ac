dnl SPDX-License-Identifier: GPL-3.0-only
AC_PREREQ([2.69])
AC_INIT([jobmarshal], [0.1.0], [https://github.com/jmmv/jobmarshal/issues],
        [jobmarshal], [https://github.com/jmmv/jobmarshal])
AC_CONFIG_AUX_DIR([admin])
AC_CONFIG_SRCDIR([Cargo.toml])
AM_INIT_AUTOMAKE([1.16 foreign no-dependencies])
AC_PATH_PROG([CARGO], [cargo])
AS_IF([test -z "$CARGO"], [AC_MSG_ERROR([cargo is required])])
AC_CONFIG_FILES([Makefile])
AC_OUTPUT
