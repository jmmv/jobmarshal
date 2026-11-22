#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
set -eu
make_cmd=$1
archive=$2
target=$3
work=$(mktemp -d)
cross_work=
cleanup() {
    if test -n "$cross_work"; then rm -rf "$cross_work"; fi
    rm -rf "$work"
}
trap cleanup EXIT HUP INT TERM

"$make_cmd" dist > "$work/dist.log" 2>&1
mkdir "$work/source" "$work/source with spaces"
tar -xzf "$archive" -C "$work/source" --strip-components=1
tar -xzf "$archive" -C "$work/source with spaces" --strip-components=1
source_dir=$work/source
# Reuse artifacts through a path with spaces to exercise Cargo output-path quoting.
ln -s "$target" "$work/target with spaces"
target=$work/target\ with\ spaces

run_make() {
    "$make_cmd" "$@" cargo_target_dir="$target"
}

configure_ok() {
    label=$1
    expected=$2
    shift 2
    mkdir "$work/$label"
    (
        cd "$work/$label" &&
            "$source_dir/configure" --prefix=/opt/jobmarshal "$@" > configure.log 2>&1
    )
    grep -Fqx "configure: selected wrappers: $expected" "$work/$label/configure.log"
}

configure_bad() {
    label=$1
    shift
    mkdir "$work/$label"
    if (cd "$work/$label" && "$source_dir/configure" "$@" > configure.log 2>&1); then
        echo "Configure unexpectedly accepted: $*" >&2
        exit 1
    fi
    grep -F 'wrapper' "$work/$label/configure.log" > /dev/null
}

configure_ok default none
configure_ok disabled none --disable-wrappers
configure_ok empty none --enable-wrappers=
configure_ok single cargo --enable-wrappers=cargo
configure_ok literal-yes yes --enable-wrappers=yes
configure_ok literal-no no --enable-wrappers=no
configure_ok multiple 'cargo cc mold' --enable-wrappers='mold cargo cc cargo'
configure_ok whitespace 'cargo cc mold' "--enable-wrappers=$(printf 'mold\tcargo\ncc cargo')"
configure_ok punctuation 'C++ cc-1.2 mold+debug' --enable-wrappers='mold+debug cc-1.2 C++'
configure_ok last-disabled none --enable-wrappers=cargo --disable-wrappers
configure_ok last-enabled cargo --disable-wrappers --enable-wrappers=cargo
configure_bad bare --enable-wrappers
configure_bad last-bare --enable-wrappers=cargo --enable-wrappers
index=0
# These invalid names intentionally contain literal shell metacharacters.
# shellcheck disable=SC2016
for invalid in /usr/bin/cargo ../cargo .cargo -cargo _cargo 'car/go' 'c;true' 'c$argo' 'c`argo' \
    'c"argo' 'c*' '*' 'c?' '[a-z]*' jobmarshal jobmarshal-wrapper; do
    index=$((index + 1))
    configure_bad "invalid-$index" "--enable-wrappers=$invalid"
done

cd "$work/multiple"
stage=$work/stage
prefix=/opt/jobmarshal-custom
bindir=$prefix/command\ files
libexecdir=$prefix/helper\ files
docdir=$prefix/documentation
man1dir=$prefix/manuals/man1
stage_make() {
    run_make "$@" DESTDIR="$stage" prefix="$prefix" bindir="$bindir" libexecdir="$libexecdir" docdir="$docdir" man1dir="$man1dir"
}
stage_make -j4 install > install.log 2>&1
for name in cargo cc mold; do
    test ! -L "$stage$bindir/$name"
    test "$stage$bindir/$name" -ef "$stage$libexecdir/jobmarshal-wrapper"
done
stage_make install JOBMARSHAL_WRAPPERS=intruder > reinstall.log 2>&1
test ! -e "$stage$bindir/intruder"
for name in cargo cc mold; do
    test "$stage$bindir/$name" -ef "$stage$libexecdir/jobmarshal-wrapper"
done

original=$(cksum "$stage$libexecdir/jobmarshal-wrapper" "$stage$bindir/jobmarshal")
for kind in file executable symlink dangling directory; do
    rm -f "$stage$bindir/cc"
    case $kind in
        file) printf keep > "$stage$bindir/cc" ;;
        executable) printf keep > "$stage$bindir/cc"; chmod 755 "$stage$bindir/cc" ;;
        symlink) ln -s "$stage$libexecdir/jobmarshal-wrapper" "$stage$bindir/cc" ;;
        dangling) ln -s nowhere "$stage$bindir/cc" ;;
        directory) mkdir "$stage$bindir/cc" ;;
    esac
    printf 'preserve documentation' > "$stage$docdir/README.md"
    if stage_make -j4 install > refusal.log 2>&1; then
        echo "Installation replaced an unrelated $kind" >&2
        exit 1
    fi
    grep -F 'Refusing to replace unrelated wrapper destination:' refusal.log > /dev/null
    test "$original" = "$(cksum "$stage$libexecdir/jobmarshal-wrapper" "$stage$bindir/jobmarshal")"
    test 'preserve documentation' = "$(cat "$stage$docdir/README.md")"
    test "$stage$bindir/cargo" -ef "$stage$libexecdir/jobmarshal-wrapper"
    case $kind in
        file|executable) test keep = "$(cat "$stage$bindir/cc")" ;;
        symlink|dangling) test -L "$stage$bindir/cc" ;;
        directory) test -d "$stage$bindir/cc"; rmdir "$stage$bindir/cc" ;;
    esac
done
rm -f "$stage$bindir/cc"
stage_make install > restore.log 2>&1
rm "$stage$bindir/cc" "$stage$bindir/mold"
printf replacement > "$stage$bindir/cc"
ln -s unrelated "$stage$bindir/mold"
stage_make uninstall > uninstall.log 2>&1
test replacement = "$(cat "$stage$bindir/cc")"
test -L "$stage$bindir/mold"
test ! -e "$stage$bindir/cargo"
test ! -e "$stage$libexecdir/jobmarshal-wrapper"
rm "$stage$bindir/cc" "$stage$bindir/mold"
test -z "$(find "$stage" ! -type d -print)"

# Change the complete selection only after uninstalling with its original configuration.
"$source_dir/configure" --prefix=/opt/jobmarshal --enable-wrappers=new-command > changed.log 2>&1
stage_make install > changed-install.log 2>&1
test "$stage$bindir/new-command" -ef "$stage$libexecdir/jobmarshal-wrapper"
for name in cargo cc mold; do test ! -e "$stage$bindir/$name"; done
stage_make uninstall > changed-uninstall.log 2>&1
test -z "$(find "$stage" ! -type d -print)"

(cd "$work/default" && run_make installcheck > installcheck.log 2>&1)
configure_ok 'build with spaces' 'cargo cc' --enable-wrappers='cargo cc'
(cd "$work/build with spaces" && run_make -j4 all debug > build.log 2>&1 && run_make installcheck > installcheck.log 2>&1)
# Automake accepts spaces in an in-tree source path; an explicit srcdir with spaces is unsafe.
(
    cd "$work/source with spaces" &&
        ./configure --prefix=/opt/jobmarshal --enable-wrappers=cargo > configure.log 2>&1 &&
        run_make -j4 all debug > build.log 2>&1 &&
        run_make installcheck > installcheck.log 2>&1
)

cross_tested=no
for candidate in /dev/shm /var/tmp /tmp; do
    test -d "$candidate" && test -w "$candidate" || continue
    cross_work=$(mktemp -d "$candidate/jobmarshal-links.XXXXXXXX")
    printf probe > "$work/probe"
    if ln "$work/probe" "$cross_work/probe" 2>/dev/null; then
        rm "$cross_work/probe"
        rmdir "$cross_work"
        cross_work=
        continue
    fi
    mkdir "$work/cross-bin"
    printf 'keep executable' > "$work/cross-bin/jobmarshal"
    if run_make -j4 install DESTDIR= prefix="$work/cross-prefix" bindir="$work/cross-bin" libexecdir="$cross_work/helpers" > cross.log 2>&1; then
        echo 'Installation unexpectedly crossed filesystems' >&2
        exit 1
    fi
    grep -F 'bindir and libexecdir must support hardlinks on the same filesystem' cross.log > /dev/null
    test 'keep executable' = "$(cat "$work/cross-bin/jobmarshal")"
    test ! -e "$cross_work/helpers/jobmarshal-wrapper"
    cross_tested=yes
    break
done
if test "$cross_tested" = no; then
    echo 'No writable second filesystem available for the cross-filesystem install check' >&2
fi
