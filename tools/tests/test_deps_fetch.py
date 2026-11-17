"""rootfs/deps.sh dep_fetch: mirror fallback, checksum enforcement, no partial files; curl is a fake, no network."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PAYLOAD = b'source archive\n'
PRIMARY = 'https://mirrors.edge.kernel.org/pub/linux/utils/kernel/kexec/kexec-tools-2.0.32.tar.xz'
MIRROR = 'https://cdn.kernel.org/pub/linux/utils/kernel/kexec/kexec-tools-2.0.32.tar.xz'

# Logs each URL; fails (curl's exit 7) for hosts listed in $FAKE_CURL_DOWN after leaving a partial file behind,
# otherwise writes $FAKE_CURL_BODY to the -o file.
FAKE_CURL = '''#!/bin/sh
out= url=
while [ $# -gt 0 ]; do
	case "$1" in
	-o) out=$2; shift ;;
	https://*) url=$1 ;;
	esac
	shift
done
echo "$url" >> "$FAKE_CURL_LOG"
host=${url#https://}; host=${host%%/*}
case " $FAKE_CURL_DOWN " in *" $host "*) printf partial > "$out"; exit 7 ;; esac
printf '%s' "$FAKE_CURL_BODY" > "$out"
'''


class DepFetch(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        for name in ('rootfs/deps.sh', 'tools/lib/portable.sh'):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, dest)
        bin_dir = self.root / 'bin'
        bin_dir.mkdir()
        (bin_dir / 'curl').write_text(FAKE_CURL)
        (bin_dir / 'curl').chmod(0o700)
        self.sources = self.root / 'rootfs/out/sources'
        self.log = self.root / 'curl.log'
        self.env = {**os.environ, 'PATH': f'{bin_dir}{os.pathsep}{os.environ["PATH"]}',
                    'FAKE_CURL_LOG': str(self.log), 'FAKE_CURL_BODY': PAYLOAD.decode(), 'FAKE_CURL_DOWN': ''}
        self.lock(hashlib.sha256(PAYLOAD).hexdigest())

    def lock(self, sha256):
        (self.root / 'rootfs/deps.lock').write_text(f'kexec-tools fork 2.0.32 {sha256} {PRIMARY}\n')

    def fetch(self, down=''):
        return subprocess.run(['sh', '-eu', '-c', 'here=$1/rootfs; . "$here/deps.sh"; dep_fetch kexec-tools',
                               'dep-fetch', str(self.root)],
                              env={**self.env, 'FAKE_CURL_DOWN': down}, capture_output=True, text=True)

    def requested(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def test_failed_primary_falls_back_to_a_mirror(self):
        result = self.fetch(down='mirrors.edge.kernel.org')
        self.assertEqual(result.returncode, 0, result.stderr)
        target = self.sources / 'kexec-tools-2.0.32.tar.xz'
        self.assertEqual(result.stdout.strip(), str(target))
        self.assertEqual(target.read_bytes(), PAYLOAD)
        self.assertEqual(self.requested(), [PRIMARY, MIRROR])
        self.assertEqual(sorted(p.name for p in self.sources.iterdir()), [target.name])

    def test_every_host_down_fails_without_leaving_a_file(self):
        result = self.fetch(down='mirrors.edge.kernel.org cdn.kernel.org www.kernel.org')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.requested()), 3)
        self.assertEqual(list(self.sources.iterdir()), [])

    def test_checksum_mismatch_fails_hard_and_tries_no_mirror(self):
        self.lock('0' * 64)
        result = self.fetch()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('wrong checksum', result.stderr)
        self.assertEqual(self.requested(), [PRIMARY])
        self.assertEqual(list(self.sources.iterdir()), [])

    def test_a_verified_file_is_not_downloaded_again(self):
        self.assertEqual(self.fetch().returncode, 0)
        self.assertEqual(self.fetch(down='mirrors.edge.kernel.org cdn.kernel.org www.kernel.org').returncode, 0)
        self.assertEqual(self.requested(), [PRIMARY])


if __name__ == '__main__':
    unittest.main()
