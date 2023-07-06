#!/bin/sh

set -e
set -u

script_dir="$(cd "$(dirname "${0}")" && pwd)"

# minimize the JSON output
"${script_dir}/target/debug/create-process-rust.exe" --json --print-args-only hallo welt 'tach moin' \
	| jq -C '{ "cmdline": .cmdline, "args": [ .args[].arg ]}'

# Call cmd.exe via a shell script
"${script_dir}/cmd.exe.sh" \
	'/c "echo moin & "%systemroot%\System32\timeout.exe" /? & "%systemroot%\System32\timeout.exe" /T 5"'
