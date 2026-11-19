"""Package the application using an explicit allowlist, never credentials or user data."""
from pathlib import Path
import hashlib
import json
import shutil
import struct
import zipfile
import tomllib
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / 'dist'
VERSION = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
DIST.mkdir(exist_ok=True)
manifest = json.loads((ROOT / '.tools/mpv/source.json').read_text(encoding='utf-8-sig'))
expected = manifest['digest'].split(':')[1]
with (ROOT / '.tools/mpv/package.7z').open('rb') as original:
    assert hashlib.file_digest(original, 'sha256').hexdigest() == expected, 'libmpv checksum mismatch'
with (ROOT / '.tools/vulkan/runtime.zip').open('rb') as original:
    assert hashlib.file_digest(original, 'sha256').hexdigest() == 'a14672efed15aafc7f5a16572d35cd3a3416eadf670aeee3cdf50ee32d5fbf83', 'Vulkan checksum mismatch'
with (ROOT / '.tools/rclone/rclone-v1.76.0-windows-amd64.zip').open('rb') as original:
    assert hashlib.file_digest(original, 'sha256').hexdigest() == '8b4fd2737ade4e70c057ef4a49deb7cffe94b0fb9d7587c189d149823f21a1dc', 'rclone checksum mismatch'

source = DIST / f'Megalodon-{VERSION}-source.zip'
with zipfile.ZipFile(source, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    for name in ['Cargo.toml', 'Cargo.lock', 'build.rs', 'desktop.ps1', 'dev.ps1', 'client.ps1', 'package.ps1', 'Instalar.ps1', 'Instalar.cmd', 'LICENSE', 'README.md', 'DESIGN.md', 'PRODUCT.md', '.gitignore', '.env.example', 'AGENTS.md']:
        archive.write(ROOT / name, name)
    for directory in ['src', 'static', 'scripts', 'docs', 'tests', '.github']:
        for path in sorted((ROOT / directory).rglob('*')):
            if path.is_file() and '__pycache__' not in path.parts:
                archive.write(path, path.relative_to(ROOT).as_posix())
    for name in ['Cargo.toml', 'Cargo.lock', 'rustc-host.cmd']:
        archive.write(ROOT / 'client' / name, 'client/' + name)
    for path in sorted((ROOT / 'client/src').rglob('*.rs')):
        archive.write(path, path.relative_to(ROOT).as_posix())

portable = DIST / f'Megalodon-{VERSION}-windows-x64-portable.zip'
with zipfile.ZipFile(portable, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    archive.write(ROOT / '.tools/rclone/rclone.exe', 'rclone.exe')
    archive.write(ROOT / '.tools/rclone/rclone-v1.76.0-windows-amd64/README.txt', 'licenses/rclone-README.txt')
    vulkan = ROOT / '.tools/vulkan/VulkanRT-X64-1.4.357.0-Components'
    archive.write(vulkan / 'x64/vulkan-1.dll', 'vulkan-1.dll')
    archive.write(vulkan / 'VulkanRT-License.txt', 'licenses/VulkanRT-License.txt')
    for origin, destination in [('target/release/megalodon-desktop.exe','Megalodon.exe'),('.tools/mpv/libmpv-2.dll','libmpv-2.dll'),('.tools/mpv/source.json','licenses/mpv-build.json'),('LICENSE','LICENSE.txt'),('docs/DESKTOP.md','LEEME.md'),('docs/THIRD-PARTY.md','licenses/THIRD-PARTY.md')]:
        archive.write(ROOT / origin, destination)
    for name in ['CASTING.md', 'DISTRIBUTION.md']:
        archive.write(ROOT / 'docs' / name, name)
    for path in sorted((ROOT / 'docs/licenses').rglob('*')):
        if path.is_file():
            archive.write(path, 'licenses/' + path.relative_to(ROOT / 'docs/licenses').as_posix())
    # Source is delivered alongside the installer; do not duplicate 50 MiB of assets inside it.

installer = DIST / f'Megalodon-{VERSION}-windows-x64-setup.exe'
with installer.open('wb') as output:
    with (ROOT / 'target/release/megalodon-setup.exe').open('rb') as stub:
        shutil.copyfileobj(stub, output)
    with portable.open('rb') as payload:
        shutil.copyfileobj(payload, output)
    output.write(struct.pack('<Q', portable.stat().st_size))
    output.write(b'MEGALODON-RUST01')

checksums = []
for path in [installer, portable, source]:
    with path.open('rb') as stream:
        checksums.append(hashlib.file_digest(stream, 'sha256').hexdigest() + '  ' + path.name)
(DIST / 'SHA256SUMS.txt').write_text('\n'.join(checksums) + '\n', encoding='ascii')
try:
    commit = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], stderr=subprocess.DEVNULL, text=True).strip()
except (OSError, subprocess.CalledProcessError):
    commit = None
(DIST / 'build-info.json').write_text(json.dumps({'commit': commit, 'version': VERSION}), encoding='utf-8')
print('\n'.join(f'{path.name}: {path.stat().st_size / 1024 / 1024:.1f} MiB' for path in [installer, portable, source]))
