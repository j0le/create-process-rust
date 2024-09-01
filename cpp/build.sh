#!/bin/sh

set -e
set -u

c++ -static --std=c++23 pargs.cpp -o pargs -Wl,--verbose -v
