"""Capture public File3dm source and readback records for nested 3D NURBS."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import time
from .client import OracleClient
from .nested_rational_probe import FAMILIES
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]
OBSERVATIONS = ROOT / 'tools/rhino_oracle/observations'


def request(readback=None):
    folder = ROOT / 'tools/rhino_oracle/fixtures/nested_rational'
    if readback is None:
        folder.mkdir(exist_ok=True)
    else:
        folder = Path(readback).resolve()
    operations = []
    for family in FAMILIES:
        for mask, gauge in [('polynomial', 1.), ('mixed', 1.), ('rational', 1.), ('mixed', 7.)]:
            name = '%s_%s_%d' % (family, mask, gauge)
            op = dict(op='nested_rational_transfer', mode='read' if readback else 'create', id=name,
                      artifact_path=str(folder / (name + '.3dm')))
            if readback is None:
                op.update(family=family, mask=mask, gauge=gauge)
            operations.append(op)
    q = dict(protocol_version=1, iterations=1,
             tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=operations)
    validate_operations(q)
    return q


def main():
    readback = sys.argv[1] if len(sys.argv) == 2 else None
    if len(sys.argv) > 2:
        raise ValueError('expected optional directory of Viboceros rewrites')
    q = request(readback)
    label = 'nested_rational_readback' if readback else 'nested_rational'
    (OBSERVATIONS / (label + '.request.json')).write_text(json.dumps(q, indent=2) + '\n')
    start = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleNestedRational20261018').run_rhino(q, timeout=240.)
    if [v['id'] for v in r['results']] != [v['id'] for v in q['operations']]:
        raise ValueError('incomplete nested rational capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    files = ('nested_rational_probe.py', 'generate_nested_rational.py', 'rhino_worker.py', 'structural_sdk.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        seconds=time.monotonic()-start, sources={f: digest(Path(__file__).with_name(f)) for f in files},
        fixtures={Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']})
    (OBSERVATIONS / (label + '.json')).write_text(json.dumps(r, indent=2, allow_nan=False) + '\n')
    print('Recorded', len(r['results']), 'public File3dm', 'readbacks' if readback else 'sources', flush=True)


if __name__ == '__main__':
    main()
