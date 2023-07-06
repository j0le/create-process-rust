#!/bin/sh

set -e
set -u

jq -C '{ "cmdline": .cmdline, "args": [ .args[].arg ]}'
