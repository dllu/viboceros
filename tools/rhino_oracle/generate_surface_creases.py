"""Capture surface crease witnesses between the legacy transverse samples."""
import argparse
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
from .client import OracleClient

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_surface_creases.py', 'surface_crease_probe.py', 'rhino_worker.py',
           'client.py', 'structural_sdk.py']


def surface(amplitude, degree=3, rational=False, shear=0.):
    h = ([0., -amplitude / 6., amplitude / 6., 0.] if degree == 3 else
         [amplitude if i == 1 else 0. for i in range(degree + 1)])
    return dict(degree_u=1, degree_v=degree, control_point_count_u=3,
                control_point_count_v=degree + 1,
                knots_u=[0., 0., 1., 2., 2.],
                knots_v=[0.] * (degree + 1) + [1.] * (degree + 1),
                control_points=[dict(point=[float(u), v / degree + (shear if u == 2 else 0.),
                                           h[v] if u == 2 else 0.],
                                     weight=(.5 + (v % 3)) if rational else 1.)
                                for v in range(degree + 1) for u in range(3)])


def request(smoke=False):
    operations = []
    cases = [('hidden', surface(.04)), ('below', surface(.02)),
             ('shear', surface(0., shear=.2)), ('narrow', surface(.02, degree=12))]
    if not smoke:
        cases += [('rational', surface(.04, rational=True)),
                  ('rational_narrow', surface(.02, degree=12, rational=True)),
                  ('flat', surface(0.)), ('large', surface(.2))]
    for name, definition in cases:
        for swap in ([False] if smoke else [False, True]):
            for scale in ([1.] if smoke else [.001, 1., 1000.]):
                d = copy.deepcopy(definition)
                for cp in d['control_points']:
                    x, y, z = cp['point']
                    cp['point'] = [scale * x, scale * y, scale * z]
                if swap:
                    nu, nv = d['control_point_count_u'], d['control_point_count_v']
                    d['control_points'] = [d['control_points'][j * nu + i] for i in range(nu) for j in range(nv)]
                    d['degree_u'], d['degree_v'] = d['degree_v'], d['degree_u']
                    d['control_point_count_u'], d['control_point_count_v'] = nv, nu
                    d['knots_u'], d['knots_v'] = d['knots_v'], d['knots_u']
                for angle in ([.1] if smoke else [.1, .5]):
                    operations.append(dict(op='surface_jets', id='surface-crease-{}-{}-{}-{}'.format(
                        name, int(swap), scale, angle), surface=copy.deepcopy(d),
                        inspect_crease=math.radians(angle)))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def holdout_request():
    operations = []
    for rational in [False, True]:
        for angle in [.1, .5]:
            for gauge in [1e-120, 1., 1e120]:
                d = surface(.02, degree=24, rational=rational)
                for cp in d['control_points']:
                    cp['point'] = [x + offset for x, offset in zip(cp['point'], [1234., -400., 75.])]
                    cp['weight'] *= gauge
                d['knots_u'] = [-4., -4., 1., 9., 9.]
                d['knots_v'] = [2.] * 25 + [7.] * 25
                operations.append(dict(op='surface_jets', id='surface-crease-holdout-{}-{}-{}'.format(
                    int(rational), angle, gauge), surface=d, inspect_crease=math.radians(angle)))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def witness_request():
    query = holdout_request()
    operations = [o for o in query['operations'] if o['id'].endswith('-1.0')]
    narrow = next(o for o in request()['operations']
                  if o['id'] == 'surface-crease-rational_narrow-0-1.0-0.5')
    operations.append(copy.deepcopy(narrow))
    for op in operations:
        op['id'] = op['id'].replace('surface-crease-', 'surface-crease-witness-', 1)
        d = op['surface']
        p = d['degree_v']
        v = d['knots_v'][p] + (d['knots_v'][-p-1] - d['knots_v'][p]) / p
        u = d['knots_u'][2]
        op['samples'] = [dict(parameter=[u, v], side_u=side) for side in ['left', 'right']]
    query['operations'] = operations
    return query


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--smoke', action='store_true')
    modes.add_argument('--holdouts', action='store_true')
    modes.add_argument('--witnesses', action='store_true')
    args = parser.parse_args()
    family = 'surface_creases_smoke' if args.smoke else 'surface_creases'
    if args.holdouts: family = 'surface_creases_holdouts'
    if args.witnesses: family = 'surface_creases_witnesses'
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if output.exists(): raise ValueError('successful crease captures are immutable')
    query = request(args.smoke)
    if args.holdouts: query = holdout_request()
    if args.witnesses: query = witness_request()
    fixture.write_text(json.dumps(query, indent=2) + '\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    print('Capturing {} surface crease cases'.format(len(query['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleSurfaceCreases20261025').run_rhino(query, timeout=1200)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('crease producer changed during capture')
    if [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]:
        raise ValueError('crease capture coverage differs')
    data['provenance'] = dict(request_sha256=digest(fixture), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
        if not archive.exists(): archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
