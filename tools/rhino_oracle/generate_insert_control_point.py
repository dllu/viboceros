"""Actual insertion commands, parameter charts, trims and extension choices."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder = ROOT / 'tools/rhino_oracle/fixtures/insert_control_point_command'
    folder.mkdir(exist_ok=True)
    rows = []
    def add(name, family, actions, **kw):
        rows.append(dict(op='insert_control_point_command', id=name, family=family,
                         actions=actions, artifact_path=str(folder / (name + '.3dm')), **kw))
    for family in ('cubic', 'rational', 'periodic', 'circle', 'ellipse', 'line', 'polyline', 'polycurve'):
        add(family, family, [dict(point=.4)])
        add(family + '_midpoint', family, ['Midpoint=Yes', dict(point=.4)])
        add(family + '_twice', family, [dict(point=.25), dict(point=.7)])
    add('cubic_cancel', 'cubic', [], cancel=True)
    add('cubic_cancel_after', 'cubic', [dict(point=.4)], cancel=True)
    add('cubic_empty', 'cubic', [])
    add('cubic_post', 'cubic', [dict(point=.4)], preselect=False)
    add('cubic_start', 'cubic', [dict(point=0.)])
    add('cubic_end', 'cubic', [dict(point=1.)])
    add('cubic_start_midpoint', 'cubic', ['Midpoint=Yes', dict(point=0.)])
    add('domain', 'cubic', [dict(point=.4)], domain=[-3., 17.])
    add('seeded', 'cubic', [dict(point=.4)], seed=True)
    add('elevated', 'cubic', [dict(point=.4)], elevate=5)
    add('negative_gauge', 'rational', [dict(point=.4)], gauge=-8.)
    add('tilted', 'cubic', [dict(point=.4)], tilted=True)
    add('reversed', 'cubic', [dict(point=.4)], reverse=True)
    add('off_curve', 'cubic', [dict(world=[2., 2., 1.])])
    add('reverse_polycurve', 'polycurve', [dict(point=.4)], reverse=True)
    add('domain_polycurve', 'polycurve', [dict(point=.4)], domain=[-3., 17.])
    for family in ('surface', 'surface_rational', 'trimmed_rectangle', 'trimmed_hole'):
        for direction in ('U', 'V'):
            add(family + '_' + direction, family, [dict(point=[.4, .6])], direction=direction)
        for direction in ('U', 'V'):
            add(family + '_midpoint_' + direction, family,
                ['Midpoint=Yes', dict(point=[.4, .6])], direction=direction)
        add(family + '_twice', family, [dict(point=[.25, .3]), dict(point=[.7, .8])])
        add(family + '_cancel_after', family, [dict(point=[.4, .6])], cancel=True)
        add(family + '_post', family, [dict(point=[.4, .6])], preselect=False)
    add('surface_toggle', 'surface', ['Toggle', dict(point=[.4, .6])])
    for direction in ('U', 'V'):
        for side in (0., 1.):
            uv = [.5, side] if direction == 'U' else [side, .5]
            add('surface_extend_' + direction + '_' + str(int(side)), 'surface',
                ['Extend', dict(point=uv)], direction=direction)
    add('surface_extend_interior', 'surface', ['Extend', dict(point=[.4, .6])])
    add('surface_extend_empty', 'surface', ['Extend'])
    add('surface_extend_cancel', 'surface', ['Extend'], cancel=True)
    add('surface_boundary_U', 'surface', [dict(point=[.5, 0.])])
    add('surface_boundary_V', 'surface', [dict(point=[0., .5])], direction='V')
    for axis in ('U', 'V'):
        for direction in ('U', 'V'):
            add('periodic_surface_' + axis + '_' + direction, 'surface',
                [dict(point=[.4, .6])], periodic_surface_axis=axis, direction=direction)
    q = dict(protocol_version=1, iterations=1,
             tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=rows)
    validate_operations(q)
    return q


def main():
    q = request(); folder = ROOT / 'tools/rhino_oracle/observations'
    (folder / 'insert_control_point_command.request.json').write_text(json.dumps(q, indent=2) + '\n')
    started = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleInsertControl20261019').run_rhino(q, timeout=600.)
    if [row['id'] for row in r['results']] != [row['id'] for row in q['operations']]:
        raise ValueError('incomplete insertion capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('generate_insert_control_point.py', 'insert_control_point_probe.py',
             'remove_knot_command_probe.py', 'insert_kink_probe.py', 'join_probe.py',
             'merge_edges_probe.py', 'rhino_worker.py', 'structural_sdk.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        seconds=time.monotonic() - started, sources={name: digest(Path(__file__).with_name(name)) for name in names},
        fixtures={Path(op['artifact_path']).name: digest(Path(op['artifact_path'])) for op in q['operations']})
    (folder / 'insert_control_point_command.json').write_text(json.dumps(r, indent=2, allow_nan=False) + '\n')
    print('Recorded', len(r['results']), 'actual InsertControlPoint commands', flush=True)


if __name__ == '__main__':
    main()
