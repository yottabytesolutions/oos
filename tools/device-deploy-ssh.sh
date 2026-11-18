#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# Compatibility entry point for the original positional SSH deployment command.
set -eu
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
host=${1:?usage: device-deploy-ssh.sh HOST [--restore]}
shift
exec "$here/device-deploy.sh" --host "$host" "$@"
