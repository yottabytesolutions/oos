"""Private host server lifecycle, with verified child/build readiness."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

import output


class HostError(ValueError):
    """A verified host could not be started or identified."""


class Host:
    def __enter__(self):
        try:
            return self.start()
        except (ValueError, OSError, subprocess.SubprocessError) as exc:
            raise HostError(str(exc)) from exc

    def start(self):
        binary = output.target() / "debug/oos-ui"
        record = output.verify(binary)
        self.temp = tempfile.TemporaryDirectory(prefix="host-", dir=output.target())
        run = Path(self.temp.name)
        self.proc = None
        try:
            (run / "store").mkdir()
            self.log = (run / "stderr").open("w+", errors="replace")
            self.proc = subprocess.Popen(
                [str(binary), "--serve", "--store-dir", str(run / "store")],
                env={**os.environ, "OOS_DEMO_LOOP": "3", "OOS_HARNESS_BUILD": record["sha256"],
                     "OOS_HARNESS_READY": str(run / "ready.json")},
                stdout=subprocess.DEVNULL, stderr=self.log)
            self.ready = wait_ready(self.proc, run / "ready.json", record["sha256"])
            output.verify(binary)
            return self
        except BaseException as exc:
            # Preserve the child's diagnostics before closing the log and deleting its run directory.
            detail = self.stderr_tail()
            self.__exit__(None, None, None)
            if isinstance(exc, (ValueError, OSError, subprocess.SubprocessError)):
                raise HostError(f"{exc}\nchild stderr (tail):\n{detail}") from exc
            raise

    def stderr_tail(self):
        if not hasattr(self, "log"):
            return "(child did not start)"
        self.log.seek(0, os.SEEK_END)
        self.log.seek(max(0, self.log.tell() - 8192))
        return "\n".join(self.log.read().splitlines()[-20:]) or "(empty)"

    def results_dir(self):
        """Create a persistent report directory next to this host's private run directory."""
        return tempfile.mkdtemp(prefix="host-results-", dir=Path(self.temp.name).parent)

    def __exit__(self, *_):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        if hasattr(self, "log"):
            self.log.close()
        self.temp.cleanup()


def wait_ready(proc, path, build, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise HostError(f"host child exited before readiness (status {proc.returncode}); no tests started")
        if path.exists():
            ready = json.loads(path.read_text())
            ports = [ready.get(k) for k in ("web", "scpi", "vxi_tcp", "vxi_udp", "vxi_core")]
            if (ready.get("pid") != proc.pid or ready.get("build") != build or
                    not all(type(p) is int and 0 < p <= 65535 for p in ports)):
                raise HostError("readiness does not identify the expected child, build and ports")
            return ready
        # Poll an atomic readiness file, never assume startup completes after a fixed delay.
        time.sleep(0.02)
    raise HostError("host child did not become ready within the deadline")
