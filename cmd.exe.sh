#!/bin/sh

set -e
set -u

script_dir="$(cd "$(dirname "${0}")" && pwd)"


cmd_path_cygwin="$(which cmd.exe)"
cmd_path_windows="$(cygpath -wa "${cmd_path_cygwin}")"
cmd_path_quoted='"'"${cmd_path_windows}"'"'

MSYS_NO_PATHCONV=1 "${script_dir}/target/debug/create-process-rust.exe" \
	--print-args --program "${cmd_path_windows}" --cmd-line-in-arg "${cmd_path_quoted} ${1}"
