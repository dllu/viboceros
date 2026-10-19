"""Owned world-point neighborhoods around retained surface getter differences."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    capture = json.loads((ROOT / 'tools/rhino_oracle/observations/remove_control_point_command.json').read_text())
    source = {r['id']: r['value'] for r in capture['results']}
    folder = ROOT / 'tools/rhino_oracle/fixtures/remove_control_point_neighbors'
    folder.mkdir(exist_ok=True)
    rows = []
    centers = ('surface_grid_U_1_2', 'surface_grid_V_1_1', 'surface_grid_V_2_1',
               'surface_grid_U_2_3', 'surface_grid_U_1_1', 'surface_grid_V_1_2')
    for index, name in enumerate(centers):
        v = source[name]
        center = v['picks'][0]
        def add(suffix, point):
            label = 'center_' + str(index) + '_' + suffix
            rows.append(dict(v['recipe'], id=label, actions=[dict(point=point)],
                             artifact_path=str(folder / (label + '.3dm'))))
        add('exact_a', center)
        add('exact_b', center)
        for scale, epsilon in enumerate((1e-13, 1e-6, .025)):
            for axis in range(3):
                for sign in (-1, 1):
                    point = list(center)
                    point[axis] += sign * epsilon
                    add('s%d_a%d_%s' % (scale, axis, 'minus' if sign < 0 else 'plus'), point)
    q = dict(protocol_version=1, iterations=1,
             tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=rows)
    validate_operations(q)
    return q


def main():
    q = request()
    folder = ROOT / 'tools/rhino_oracle/observations'
    (folder / 'remove_control_point_neighbors.request.json').write_text(json.dumps(q, indent=2) + '\n')
    started = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleControlNeighbors20261019').run_rhino(q, timeout=600.)
    if [v['id'] for v in r['results']] != [v['id'] for v in q['operations']]:
        raise ValueError('incomplete neighborhood capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('generate_remove_control_point_neighbors.py', 'generate_remove_control_point.py',
             'remove_control_point_probe.py', 'remove_knot_command_probe.py', 'insert_kink_probe.py',
             'rhino_worker.py', 'structural_sdk.py', 'join_probe.py', 'merge_edges_probe.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip(),
                           seconds=time.monotonic()-started,
                           sources={n:digest(Path(__file__).with_name(n)) for n in names},
                           fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder / 'remove_control_point_neighbors.json').write_text(json.dumps(r, indent=2, allow_nan=False) + '\n')
    print('Recorded', len(r['results']), 'control-removal neighborhoods', flush=True)


if __name__ == '__main__':
    main()
