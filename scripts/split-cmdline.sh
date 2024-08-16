#!/bin/sh

set -e
set -u

script_dir="$(cd "$(dirname "${0}")" && pwd)"

cpr_fn(){
	MSYS_NO_PATHCONV=1 "${script_dir}/../target/debug/create-process-rust.exe" "${@}"
}

cpr_fn \
	--json \
	--split-and-print-inner-cmdline \
	--dry-run \
	--program-is-null \
	--cmd-line-in-arg "prog ${1}" \
	2>/dev/null \
	| jq '[.args[1:][].arg]'

