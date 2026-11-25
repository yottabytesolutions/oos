# SPDX-License-Identifier: GPL-2.0-or-later
"""Keep the worker alive for SCPI restoration after a terminal interrupt."""

import signal
import subprocess
import sys

worker = None
interrupted = None


def interrupt(signum, _frame):
    global interrupted
    if interrupted is None:
        interrupted = signum
        if worker is not None:
            worker.stdin.close()


for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(sig, interrupt)

# Terminal signals target the foreground process group. The worker receives
# cancellation through its input pipe and stays alive to restore the settings.
worker = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE, start_new_session=True)
if interrupted is not None:
    worker.stdin.close()
status = worker.wait()
if interrupted is not None:
    sys.exit(128 + interrupted)
sys.exit(status if status >= 0 else 128 - status)
