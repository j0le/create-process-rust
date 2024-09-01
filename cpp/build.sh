#!/bin/sh

set -e
set -u

mt='C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\mt.exe'

if test -e pargs.o;        then rm pargs.o;        fi
if test -e pargs.exe;      then rm pargs.exe;      fi
if test -e pargs-utf8.exe; then rm pargs-utf8.exe; fi

c++ -static --std=c++23 pargs.cpp -c -o pargs.o
c++ -static --std=c++23 pargs.o -o pargs.exe
c++ -static --std=c++23 pargs.o -o pargs-utf8.exe \
	-Xlinker --Xlink=/manifest:EMBED \
	-Xlinker --Xlink=/manifestinput:UTF8Manifest.xml \
	-v -Wl,--verbose
