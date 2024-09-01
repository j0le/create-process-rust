#!/bin/sh

set -e
set -u

script_path="$(dirname "${0}")"

mt='C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\mt.exe'
build_dir="${script_path}/build.d"

if test -e "${build_dir}"; then
	rm -r "${build_dir}"
fi
mkdir "${build_dir}"

cd "${build_dir}"

c++ -static --std=c++23 "..\pargs.cpp" -c -o pargs.o
c++ -static --std=c++23 pargs.o -o pargs.exe
c++ -static --std=c++23 pargs.o -o pargs-utf8.exe \
	-Xlinker --Xlink=-manifest:EMBED \
	-Xlinker '--Xlink=-manifestinput:..\UTF8Manifest.xml' \
	-v -Wl,--verbose
