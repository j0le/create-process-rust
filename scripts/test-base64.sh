#!/bin/sh

set -e
set -u

script_dir="$(dirname "${0}")"

cmd_path="$(which cmd.exe)"
win_path="$(cygpath -w -a "${cmd_path}")"
printf '%s' "${win_path}"
printf '\n'

printf '%s' "${win_path}" | iconv -f UTF-8 -t UTF-16LE | base64 --wrap=0
printf '\n'
base64_path="$(printf '%s' "${win_path}" | iconv -f UTF-8 -t UTF-16LE | base64 --wrap=0)"

printf '%s' "${win_path}" | iconv -f UTF-8 -t UTF-16LE | base64 --wrap=0 | base64 -d | iconv -f UTF-16LE -t UTF-8
printf '\n'

cpr="${script_dir}/../cpr.exe"

"${cpr}" --dry-run --program-utf16le-base64 "${base64_path}" --prepend-program --cmd-line-in-arg '/c (echo hello)'
