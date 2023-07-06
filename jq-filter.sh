#!/bin/sh

set -e
set -u

#script_dir="$(cd dirname "${0}" && pwd)"

jq -C '{ "cmdline": .cmdline, "args": [ .args[].arg ]}'
