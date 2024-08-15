#!/bin/sh

set -e
set -u

script_dir="$(cd "$(dirname "${0}")" && pwd)"

cpr(){
	MSYS_NO_PATHCONV=1 "${script_dir}/../target/debug/create-process-rust.exe" "${@}"
}


cpr --json --split-and-print-inner-cmdline --dry-run --program-is-null --cmd-line-in-arg 'prog "hello world"' \
	2>/dev/null \
	| jq '.args[].arg' -C \
	| less -R
printf '%s\n' '--------------------------'

cpr --json --print-args --split-and-print-inner-cmdline --dry-run --program-is-null --cmd-line-in-arg 'prog "hello world"' \
	2>/dev/null \
	| jq . -C \
	| less -R
printf '%s\n' '--------------------------'

cpr --split-and-print-inner-cmdline --dry-run --program-is-null --cmd-line-in-arg 'prog "hello world"'
printf '%s\n' '--------------------------'
