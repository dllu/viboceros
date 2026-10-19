"""Reproducible actual control-removal commands, distinct from deleting grips."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_knot_command import ROOT
from .structural_sdk import validate_operations


def request():
    folder = ROOT / 'tools/rhino_oracle/fixtures/remove_control_point_command'
    folder.mkdir(exist_ok=True)
    rows = []
    def add(name, family, actions, **kw):
        rows.append(dict(op='remove_control_point_command', id=name, family=family,
                         actions=actions, seed=kw.pop('seed_override',False), artifact_path=str(folder / (name + '.3dm')), **kw))
    for family in ('cubic', 'rational', 'periodic', 'circle', 'ellipse', 'line', 'polyline', 'polycurve'):
        add(family, family, [dict(grip=1)])
        add(family + '_start', family, [dict(grip=0)])
        add(family + '_cancel', family, [], cancel=True)
        add(family + '_empty', family, [])
        add(family + '_post', family, [dict(grip=1)], preselect=False)
    for family in ('surface', 'surface_rational', 'trimmed_rectangle', 'trimmed_hole'):
        for direction in ('U', 'V'):
            add(family + '_' + direction, family, [dict(grip=[1, 1])], direction=direction)
        add(family + '_toggle', family, ['Toggle', dict(grip=[1, 1])])
        add(family + '_cancel', family, [], cancel=True)
        add(family + '_empty', family, [])
        add(family + '_post', family, [dict(grip=[1, 1])], preselect=False)
    add('polysurface', 'box', [dict(grip=[1, 1])], face=2)
    add('elevated', 'cubic', [dict(grip=2)], elevate=5)
    add('domain', 'rational', [dict(grip=1)], domain=[-3., 17.])
    add('negative_gauge', 'rational', [dict(grip=1)], gauge=-8.)
    add('tilted', 'cubic', [dict(grip=1)], tilted=True)
    add('off_curve', 'cubic', [dict(point=[2., 2., 1.])])
    for family in ('cubic', 'rational', 'periodic', 'polyline'):
        add(family + '_seeded', family, [dict(grip=3)], seed_override=True)
    for axis in ('U', 'V'):
        for direction in ('U', 'V'):
            add('periodic_surface_' + axis + '_' + direction, 'surface', [dict(grip=[1, 1])], periodic_surface_axis=axis, direction=direction)
    for family, size in (('surface',4),('surface_rational',3)):
        for direction in ('U','V'):
            for u in range(size):
                for v in range(size):
                    add(f'{family}_grid_{direction}_{u}_{v}', family, [dict(grip=[u,v])], direction=direction)
    for i in (2,3,4,5,6):
        add('polycurve_grip_'+str(i), 'polycurve', [dict(grip=i)])
    q = dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=rows)
    validate_operations(q)
    return q


def main():
    q = request(); folder = ROOT / 'tools/rhino_oracle/observations'
    (folder / 'remove_control_point_command.request.json').write_text(json.dumps(q, indent=2) + '\n')
    started = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleRemoveControl20261019').run_rhino(q, timeout=600.)
    if [x['id'] for x in r['results']] != [x['id'] for x in q['operations']]:
        raise ValueError('incomplete control removal capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('remove_control_point_probe.py', 'generate_remove_control_point.py', 'remove_knot_command_probe.py',
             'generate_remove_knot_command.py', 'insert_kink_probe.py', 'rhino_worker.py',
             'structural_sdk.py', 'join_probe.py', 'merge_edges_probe.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                           seconds=time.monotonic()-started, sources={n: digest(Path(__file__).with_name(n)) for n in names},
                           fixtures={Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder / 'remove_control_point_command.json').write_text(json.dumps(r, indent=2, allow_nan=False) + '\n')
    print('Recorded', len(r['results']), 'actual RemoveControlPoint commands', flush=True)


if __name__ == '__main__':
    main()
