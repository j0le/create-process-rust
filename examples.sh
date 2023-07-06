#!/bin/sh

set -e
set -u

script_dir="$(cd "$(dirname "${0}")" && pwd)"

"${script_dir}/target/debug/create-process-rust.exe" --json --print-args-only hallo welt 'tach moin' \
	| jq -C '{ "cmdline": .cmdline, "args": [ .args[].arg ]}'
