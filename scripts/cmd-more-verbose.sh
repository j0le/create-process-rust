#!/bin/sh

set -e
set -u
set -x

script_dir="$(cd "$(dirname "${0}")" && pwd)"


cmd_path_cygwin="$(which cmd.exe)"
cmd_path_windows="$(cygpath -wa "${cmd_path_cygwin}")"
#cmd_path_quoted='"'"${cmd_path_windows}"'"'

MSYS_NO_PATHCONV=1 "${script_dir}/../target/debug/create-process-rust.exe" \
	--print-args --split-and-print-inner-cmdline --program "${cmd_path_windows}" --prepend-program --cmd-line-in-arg "${1}"
