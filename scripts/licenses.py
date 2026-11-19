"""Collect license texts from the downloaded, locked Rust crates for the local package."""
from pathlib import Path
import json
import shutil
import tomllib
import urllib.request
import os

root = Path(__file__).resolve().parents[1]
destination = root / 'docs/licenses'
destination.mkdir(parents=True, exist_ok=True)
packages = tomllib.loads((root / 'Cargo.lock').read_text())['package']
packages += tomllib.loads((root / 'client/Cargo.lock').read_text())['package']
homes = [root / '.tools/cargo', Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))]
registries = [registry for home in homes for registry in (home / 'registry/src').glob('*')]
inventory = []
seen = set()
for package in packages:
    name, version = package['name'], package['version']
    if (name, version) in seen:
        continue
    seen.add((name, version))
    if not package.get('source', '').startswith('registry+'):
        continue
    source = next((reg / f'{name}-{version}' for reg in registries if (reg / f'{name}-{version}').is_dir()), None)
    if source is None:
        continue
    info = tomllib.loads((source / 'Cargo.toml').read_text())['package']
    inventory.append({key: info.get(key) for key in ['name', 'version', 'license', 'repository']})
    for path in source.iterdir():
        if path.is_file() and path.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE', 'COPYRIGHT')):
            target = destination / 'rust' / f'{name}-{version}' / path.name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
(destination / 'rust-dependencies.json').write_text(json.dumps(inventory, indent=2), encoding='utf-8')
for filename in ['Copyright', 'LICENSE.GPL', 'LICENSE.LGPL']:
    target = destination / ('mpv-' + filename + '.txt')
    if not target.exists():
        urllib.request.urlretrieve('https://raw.githubusercontent.com/mpv-player/mpv/bcde6b93cc/' + filename, target)
print(f'Collected notices for {len(inventory)} Rust packages and mpv.')
