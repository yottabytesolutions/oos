"""The coordinator image suite used by check --image before unit rounds and releases. Never accesses a device."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import output
import build_profiles
from check_inventory import run
from image_validate import validate


def kernel_source():
    expected, name = (output.ROOT / 'kernel/base.sha256').read_text().split()
    source = output.ROOT / 'kernel' / name
    if not source.exists():
        source.parent.mkdir(parents=True, exist_ok=True)
        repository = os.environ.get('GITHUB_REPOSITORY')
        if not repository:
            raise ValueError('missing checksum-pinned kernel source; supply kernel/src/linux-6.12.115-base.tar.xz (kernel/README.md)')
        run(['gh', 'release', 'download', '--repo', repository, '--pattern', source.name, '--dir', source.parent])
    build_profiles.kernel_source()
    if hashlib.sha256(source.read_bytes()).hexdigest() != expected:
        raise ValueError('kernel source checksum mismatch')


def main():
    os.environ['CARGO_TARGET_DIR'] = str(output.target())
    # Validation must use public synthetic inputs even if an interactive shell has unit defaults set.
    for name in ('OOS_CAL_DIR', 'OOS_UNIT', 'OOS_GEL_OLD', 'OOS_SIGNING_KEY', 'OOS_BOARD', 'OOS_KBASE', 'OOS_OUT',
                 'OOS_NO_AUTOACQ'):
        os.environ.pop(name, None)
    kernel_source()
    run(['cargo', 'xtask', 'doctor', '--image'])
    for command in (['sh', 'kernel/build-base.sh'], ['sh', 'boot/build-dev-image.sh'],
                    ['sh', 'boot/sign-dev-image.sh', '--community'], ['python3', '-I', 'boot/build-gel.py']):
        run(command)
    validate()
    image = output.target() / 'image'
    with (image / 'manifest.txt').open('wb') as manifest:
        subprocess.run(['python3', '-I', 'tools/image-manifest.py', 'kernel/out/base/Image',
                        str(image / 'oos-dho900.dtb'), str(image / 'initramfs.cpio.gz')],
                       cwd=output.ROOT, stdout=manifest, check=True)
    for script in ('unit-dtb-check', 'gel-sim', 'usb-sim', 'wlan-sim'):
        run(['sh', f'tools/{script}.sh'])
    print('local image suite passed; no hardware accessed')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f'image-suite failed: {exc}; prepare tools/bootstrap.sh --image')
