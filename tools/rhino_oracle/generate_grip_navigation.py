"""Reproducible directional control/edit grip selection command witnesses."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .grip_navigation_probe import COMMANDS, KINDS
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]


def request():
    folder = ROOT / 'tools/rhino_oracle/fixtures/grip_navigation'
    folder.mkdir(exist_ok=True)
    actions = [dict(command=c, seed=seed) for seed in ('first', 'last', 'middle', 'ends', 'all', 'empty') for c in COMMANDS]
    actions += [dict(command=c, **({'seed': 'first'} if i == 0 else {}))
                for i, c in enumerate(('NextU', 'NextU', 'PrevU', 'AddNextV', 'AddPrevU', 'SelV'))]
    operations = []
    for kind in KINDS:
        for mode in ('control', 'edit'):
            if mode == 'edit' and (kind.startswith('surface') or kind == 'mesh'): continue
            name = kind + '_' + mode
            operations.append(dict(op='grip_navigation', id=name,
                sources=[dict(kind=kind, mode=mode, parent=False)], actions=actions,
                artifact_path=str(folder / (name + '.3dm'))))
    for parent in (False, True):
        name = 'mixed_' + str(int(parent))
        operations.append(dict(op='grip_navigation', id=name,
            sources=[dict(kind=kind, mode=mode, parent=parent) for kind, mode in
                     (('periodic', 'control'), ('surface_closed', 'control'), ('curve', 'edit'), ('mesh', 'control'))],
            actions=actions, artifact_path=str(folder / (name + '.3dm'))))
    q = dict(protocol_version=1, iterations=1, operations=operations)
    validate_operations(q)
    return q


def main():
    q = request(); folder = ROOT / 'tools/rhino_oracle'
    path = folder / 'fixtures/grip_navigation.json'
    path.write_text(json.dumps(q, indent=2) + '\n')
    r = OracleClient(settings_scheme='VibocerosOracleGripNavigation20261019').run_rhino(q, timeout=1200.)
    if [o['id'] for o in q['operations']] != [o['id'] for o in r['results']]: raise ValueError('incomplete navigation capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('generate_grip_navigation.py', 'grip_navigation_probe.py', 'grip_transform_probe.py',
             'join_probe.py', 'merge_edges_probe.py', 'rhino_worker.py', 'structural_sdk.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        sources={n: digest(Path(__file__).with_name(n)) for n in names}, request_sha256=digest(path),
        fixtures={Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder / 'observations/grip_navigation.json').write_text(json.dumps(r, indent=2, allow_nan=False) + '\n')
    for op in q['operations']:
        Path(op['artifact_path'] + '.capture.json').unlink()
    print('Recorded', len(r['results']), 'grip navigation workflows', flush=True)


if __name__ == '__main__': main()
