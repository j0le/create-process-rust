#!/bin/sh

set -e
set -u


encode() {
	iconv -f UTF-8 -t UTF-16LE | base64 --wrap=0
}
decode() {
	base64 -d | iconv -f UTF-16LE -t UTF-8
}

script_dir="$(dirname "${0}")"

cmd_path="$(which cmd.exe)"
win_path="$(cygpath -w -a "${cmd_path}")"
printf '%s' "${win_path}"
printf '\n'

printf '%s' "${win_path}" | encode
printf '\n'
base64_path="$(printf '%s' "${win_path}" | encode)"

printf '%s' "${win_path}" | encode | decode
printf '\n'

cpr="${script_dir}/../cpr.exe"

cmdline='/c (echo hello)'
base64_cmdline="$(printf '%s' "${cmdline}" | encode)"
printf '%s\n%s\n' "${cmdline}" "${base64_cmdline}"

printf '%s\n' '------------------------'

MSYS_NO_PATHCONV=1 "${cpr}" --print-args --dry-run \
	--program-utf16le-base64 "${base64_path}" \
	--prepend-program \
	--cmd-line-utf16le-base64 "${base64_cmdline}"

printf '%s\n' '------------------------'

# negativ test
! MSYS_NO_PATHCONV=1 "${cpr}" --print-args --dry-run \
	--program-utf16le-base64 "öäü" \
	--prepend-program --cmd-line-in-arg "${cmdline}" # 2>/dev/null
