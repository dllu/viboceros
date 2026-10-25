"""Public Loft ordering under input permutations, reversal and rigid transforms."""
import argparse
import copy
import gzip
import hashlib
import itertools
import json
import math
from pathlib import Path
from .client import OracleClient

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_loft_ordering.py', 'client.py', 'structural_sdk.py', 'rhino_worker.py']


def reverse(curve):
    c = copy.deepcopy(curve)
    c['control_points'].reverse()
    lo, hi = c['knots'][0], c['knots'][-1]
    c['knots'] = [lo + hi - t for t in reversed(c['knots'])]
    return c


def request(smoke=False):
    def curve(points, weights=None):
        return dict(degree=3, knots=[0.] * 4 + [1.] * 4,
                    control_points=[dict(point=[x, y, 0.], weight=w) for (x, y), w in
                                    zip(points, weights or [1.] * len(points))])
    a = curve([(0., 0.), (.5, 2.), (2., -1.), (3., .5)], [1., 1.4, .7, 1.2])
    b = curve([(0., 0.), (1., 3.), (2., -4.), (3., 0.)], [1., 10., .1, 1.])
    c = curve([(0., 0.), (1., 2.), (2., -2.), (3., 0.)])
    q = dict(degree=2, knots=[0., 0., 0., 1., 1., 1.],
             control_points=[dict(point=p, weight=w) for p, w in
                             zip([[2., 0., 0.], [2., 2., 0.], [0., 2., 0.]], [1., math.sqrt(.5), 1.])])
    line = dict(degree=1, knots=[0., 0., 1., 1.], control_points=[
        dict(point=[0., 0., 0.], weight=1.), dict(point=[2., 0., 0.], weight=1.)])
    operations = []
    for name, source in [('cubic', a), ('weighted', b), ('symmetric', c), ('quarter', q), ('line', line)]:
        if smoke and name not in ['cubic', 'weighted']:
            continue
        for backwards in [False, True]:
            base = reverse(source) if backwards else source
            for axis in ([2] if smoke else [0, 1, 2]):
                profiles = []
                for index, offset in enumerate([0., 2., 5.]):
                    profile = copy.deepcopy(base)
                    for cp in profile['control_points']:
                        x, y, _ = cp['point']
                        xyz = [x * (1. + .2 * index) + .1 * index, y * (1. + .1 * index), offset]
                        cp['point'] = xyz if axis == 2 else ([xyz[2], xyz[0], xyz[1]] if axis == 0 else [xyz[1], xyz[2], xyz[0]])
                    profiles.append(profile)
                for permutation in ([(0, 1, 2), (2, 0, 1)] if smoke else itertools.permutations(range(3))):
                    operations.append(dict(op='loft', id='loft-order-{}-{}-{}-{}'.format(
                        name, int(backwards), axis, ''.join(map(str, permutation))),
                        curves=[copy.deepcopy(profiles[i]) for i in permutation], style='normal',
                        closed=False, command=True, rebuild_point_count=0,
                        inspect_input_planes=True, sample_geometry=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def line_request():
    families = [
        ('four', [(0., 2., 0.), (.1, 2.5, 2.), (.2, 3., 5.), (.3, 3.5, 9.)]),
        ('spread', [(0., 2., 0.), (100., 102., 2.), (0., 2., 5.)]),
        ('nested', [(0., 100., 0.), (-1., 1., 2.), (49., 51., 5.)]),
        ('wide', [(-100., 100., 0.), (-2., 2., 1.), (1., 3., 4.), (2., 4., 8.)]),
    ]
    operations = []
    for name, positions in families:
        for backwards in [False, True]:
            curves = []
            for lo, hi, z in positions:
                curve = dict(degree=1, knots=[0., 0., 1., 1.], control_points=[
                    dict(point=[lo, 0., z], weight=1.), dict(point=[hi, 0., z], weight=1.)])
                curves.append(reverse(curve) if backwards else curve)
            for order in itertools.permutations(range(len(curves))):
                operations.append(dict(op='loft', id='loft-line-order-{}-{}-{}'.format(
                    name, int(backwards), ''.join(map(str, order))),
                    curves=[copy.deepcopy(curves[i]) for i in order], style='normal',
                    closed=False, command=True, rebuild_point_count=0,
                    inspect_input_planes=True, sample_geometry=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def holdout_request():
    operations = []
    for family in ['spread_cubic', 'same_plane_cubic', 'rational_line', 'cubic_line']:
        curves = []
        for index in range(3):
            if family.endswith('cubic'):
                curve = copy.deepcopy(request(True)['operations'][0]['curves'][index])
                for cp in curve['control_points']:
                    cp['point'][0] += [0., 100., 0.][index]
                    if family == 'same_plane_cubic': cp['point'][2] = 0.
            else:
                positions = ([(0., 100., 0.), (39., 41., 2.), (89., 91., 5.)]
                             if family == 'rational_line' else
                             [(0., 100., 0.), (-1., 1., 2.), (49., 51., 5.)])
                lo, hi, z = positions[index]
                degree = 1 if family == 'rational_line' else 3
                xs = [lo, hi] if degree == 1 else [lo, lo, lo, hi]
                weights = [1., 10.] if degree == 1 else [1.] * 4
                curve = dict(degree=degree, knots=[0.] * (degree + 1) + [1.] * (degree + 1),
                             control_points=[dict(point=[x, 0., z], weight=w) for x, w in zip(xs, weights)])
            curves.append(curve)
        for backwards in [False, True]:
            profiles = [reverse(c) if backwards else c for c in curves]
            for order in itertools.permutations(range(3)):
                operations.append(dict(op='loft', id='loft-order-holdout-{}-{}-{}'.format(
                    family, int(backwards), ''.join(map(str, order))),
                    curves=[copy.deepcopy(profiles[i]) for i in order], style='normal',
                    closed=False, command=True, rebuild_point_count=0,
                    inspect_input_planes=True, sample_geometry=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def spatial_request():
    families = []
    for height in [85., 170.]:
        curves = [dict(degree=3, knots=[0.] * 4 + [1.] * 4,
                       control_points=[dict(point=[x, y, 0.], weight=1.) for x, y in
                                       [(0., 0.), (1., 200.), (1., 200.), (2., 0.)]])]
        for y in [0., height]:
            c = copy.deepcopy(request(True)['operations'][0]['curves'][0])
            for cp in c['control_points']:
                cp['point'][0] += 10.
                cp['point'][1] += y
            curves.append(c)
        families.append(('arch' + str(int(height)), curves))
    for name in ['partial_planes', 'line_ties', 'mixed_directions', 'tilted_planes']:
        if name == 'line_ties':
            curves = []
            for z in [0., -2., 2.]:
                curves.append(dict(degree=1, knots=[0., 0., 1., 1.], control_points=[
                    dict(point=[0., 0., z], weight=1.), dict(point=[2., 0., z], weight=1.)]))
        else:
            curves = copy.deepcopy(request(True)['operations'][0]['curves'])
            if name == 'partial_planes':
                curves.append(copy.deepcopy(curves[2]))
                for i, c in enumerate(curves):
                    for cp in c['control_points']:
                        cp['point'][0] += [0., 100., 1., 2.][i]
                        cp['point'][2] = [0., 2., 2., 5.][i]
            elif name == 'mixed_directions':
                curves[1] = reverse(curves[1])
            else:
                for cp in curves[1]['control_points']:
                    cp['point'][2] += .2 * cp['point'][1]
        families.append((name, curves))
    operations = []
    for name, curves in families:
        for backwards in [False, True]:
            profiles = [reverse(c) if backwards else c for c in curves]
            for order in itertools.permutations(range(len(curves))):
                operations.append(dict(op='loft', id='loft-order-spatial-{}-{}-{}'.format(
                    name, int(backwards), ''.join(map(str, order))),
                    curves=[copy.deepcopy(profiles[i]) for i in order], style='normal',
                    closed=False, command=True, rebuild_point_count=0,
                    inspect_input_planes=True, sample_geometry=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--smoke', action='store_true')
    modes.add_argument('--lines', action='store_true')
    modes.add_argument('--holdouts', action='store_true')
    modes.add_argument('--spatial', action='store_true')
    args = parser.parse_args()
    family = 'loft_ordering_smoke' if args.smoke else 'loft_ordering'
    if args.lines: family = 'loft_line_ordering'
    if args.holdouts: family = 'loft_ordering_holdouts'
    if args.spatial: family = 'loft_ordering_spatial'
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if output.exists():
        raise ValueError('successful native ordering captures are immutable')
    query = line_request() if args.lines else request(args.smoke)
    if args.holdouts: query = holdout_request()
    if args.spatial: query = spatial_request()
    fixture.write_text(json.dumps(query, indent=2) + '\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    print('Capturing {} Loft ordering cases'.format(len(query['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleLoftOrdering20261025').run_rhino(query, timeout=1200)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('ordering producer changed during capture')
    if [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]:
        raise ValueError('ordering coverage differs')
    data['provenance'] = dict(request_sha256=digest(fixture), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
