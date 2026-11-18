# SPDX-License-Identifier: GPL-2.0-or-later
"""Submit boot-tool writes to the HAL's ADR 0011 writer (build oos-update for the host first)."""
import os
import subprocess


def write(target, area, offset, data):
    subprocess.run([os.environ.get('OOS_UPDATE', 'oos-update'), 'storage-write', area, str(offset),
                    '--host-card', str(target), '--execute'], input=data, check=True)
