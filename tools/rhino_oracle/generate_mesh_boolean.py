"""Capture immutable mesh Boolean SDK observations with frozen producers."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_boolean_cases import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_mesh_boolean.py', 'mesh_boolean_cases.py', 'mesh_boolean_probe.py',
           'client.py', 'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--controls', action='store_true')
    args = parser.parse_args()
    family = 'mesh_boolean_controls' if args.controls else 'mesh_boolean_sdk'
    output = HERE / 'observations' / (family+'.json')
    if output.exists():
        raise ValueError('native observations are immutable; add a family')
    payload = request(controls=args.controls)
    query = HERE / 'fixtures' / (family+'.json')
    query.write_text(json.dumps(payload, indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    for name, sha in sources.items():
        archive = HERE / 'observations/source_snapshots' / (sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Capturing {} mesh Boolean SDK cases'.format(len(payload['operations'])), flush=True)
    scheme = 'VibocerosOracleMeshBoolean20261024' + ('Controls' if args.controls else 'Sdk')
    data = OracleClient(settings_scheme=scheme).run_rhino(payload, timeout=240)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False)+'\n')
    print('Recorded {} mesh Boolean SDK cases'.format(len(data['results'])), flush=True)


if __name__ == '__main__':
    main()
