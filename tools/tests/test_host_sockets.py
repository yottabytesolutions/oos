"""Run outside restricted sandboxes, after tools/output.py build -p oos-ui."""
import json
from pathlib import Path
import socket
import struct
import sys
import unittest
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from host_harness import Host


class HostSockets(unittest.TestCase):
    def test_two_runs_use_their_own_children_ports_and_storage(self):
        with Host() as first, Host() as second:
            self.assertNotEqual(first.proc.pid, second.proc.pid)
            self.assertNotEqual(first.temp.name, second.temp.name)
            self.assertFalse(set(first.ready[k] for k in ("web", "scpi", "vxi_tcp", "vxi_core")) &
                             set(second.ready[k] for k in ("web", "scpi", "vxi_tcp", "vxi_core")))
            for host in (first, second):
                with socket.create_connection(("127.0.0.1", host.ready["scpi"]), timeout=5) as s:
                    s.sendall(b"*IDN?\n")
                    self.assertIn(b"OOS", s.recv(1024).upper())
                body = json.dumps(dict(jsonrpc="2.0", id=1, method="tools/list")).encode()
                req = urllib.request.Request(f'http://127.0.0.1:{host.ready["web"]}/mcp', body,
                                             {"Content-Type": "application/json"})
                with urllib.request.urlopen(req, timeout=5) as response:
                    self.assertIn("result", json.load(response))
                msg = struct.pack("!14I", 7, 0, 2, 100000, 2, 3, 0, 0, 0, 0, 0x0607AF, 1, 6, 0)
                with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
                    s.settimeout(5)
                    s.sendto(msg, ("127.0.0.1", host.ready["vxi_udp"]))
                    reply = s.recv(1024)
                    self.assertEqual(struct.unpack("!I", reply[-4:])[0], host.ready["vxi_core"])
        self.assertIsNotNone(first.proc.returncode)
        self.assertFalse(Path(first.temp.name).exists())
        self.assertFalse(Path(second.temp.name).exists())


if __name__ == "__main__":
    unittest.main()
