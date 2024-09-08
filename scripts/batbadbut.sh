#!/bin/sh

# https://flatt.tech/research/posts/batbadbut-you-cant-securely-execute-commands-on-windows/

set -e
set -u

cd "$(dirname "${0}")"

cpr_fn(){
	MSYS_NO_PATHCONV=1 "../target/debug/create-process-rust.exe" "${@}"
}

./p.bat 'hello World' '" x & calc.exe'

cpr_fn --print-args --split-and-print-inner-cmdline --program '.\p.bat' --prepend-program --cmd-line-is-rest \
	'hello World' '" x &calc.exe'

cpr_fn --print-args --split-and-print-inner-cmdline --program '.\p.bat' --prepend-program \
	--cmd-line-in-arg '"hello World" "\" x &calc.exe"'
