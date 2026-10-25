"""Capture public Loft Rebuild output and independent Curve.Rebuild definitions."""
import argparse
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
from .client import OracleClient

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_loft_rebuild.py', 'fixtures/loft.json', 'client.py',
           'rhino_worker.py', 'structural_sdk.py']


def request(smoke=False, bounds=False, commands=False):
    old = json.loads((HERE / 'fixtures/loft.json').read_text())
    circle = next(c for op in old['operations'] for c in op['curves']
                  if c['degree'] == 2 and len(c['control_points']) == 9)
    def curve(degree, points, knots, weights=None):
        return dict(degree=degree, knots=knots, control_points=[dict(point=p, weight=w)
            for p, w in zip(points, weights or [1.] * len(points))])
    quarter = curve(2, [[2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
                    [0., 0., 0., 1., 1., 1.], [1., math.sqrt(.5), 1.])
    cubic = curve(3, [[0., 0., 0.], [.5, 2., 0.], [2., -1., 0.], [3., .5, 0.]],
                  [0.] * 4 + [1.] * 4, [1., 1.4, .7, 1.2])
    polygon = curve(1, [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.], [0., 0., 0.]],
                    [0., 0., 1., 2., 3., 4., 4.])
    line = curve(1, [[0., 0., 0.], [2., 0., 0.]], [0., 0., 1., 1.])
    operations = []
    for shape, source in [('quarter', quarter), ('circle', circle), ('cubic', cubic),
                          ('polygon', polygon), ('line', line)]:
        if smoke and shape not in ['quarter', 'circle']:
            continue
        profiles = []
        for i in range(3):
            c = copy.deepcopy(source)
            for cp in c['control_points']:
                x, y, _ = cp['point']
                cp['point'] = [(1. + .3 * i) * x + .2 * i,
                               (1. + .1 * i) * y - .15 * i, 2. * i]
            profiles.append(c)
        for count in ([1] if bounds else [4, 6] if smoke else [0, 2, 3, 4, 6, 9]):
            for style in (['normal', 'straight'] if smoke else
                          ['normal', 'loose', 'tight', 'straight', 'uniform']):
                for closed in ([False] if smoke or shape not in ['circle', 'polygon'] else [False, True]):
                    operations.append(dict(op='loft', id='loft-rebuild-{}-{}-{}-{}'.format(
                        shape, count, style, int(closed)), curves=copy.deepcopy(profiles),
                        style=style, closed=closed, rebuild_point_count=count,
                        sample_geometry=True, inspect_inputs=True, inspect_rebuilt=count > 0,
                        allow_empty=True))
    if not smoke and not bounds:
        for shape, source in [('quarter', quarter), ('circle', circle)]:
            for count in [4, 6, 9]:
                for sections in [1, 3]:
                    profiles = []
                    for i in range(sections):
                        c = copy.deepcopy(source)
                        for cp in c['control_points']:
                            x, y, _ = cp['point']
                            cp['point'] = [(1. + .3 * i) * x + .2 * i,
                                           (1. + .1 * i) * y - .15 * i, 2. * i]
                        profiles.append(c)
                    for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
                        for start, end in [(True, False), (False, True), (True, True)]:
                            op = dict(op='loft', id='loft-rebuild-points-{}-{}-{}-{}-{}{}'.format(
                                shape, count, sections, style, int(start), int(end)),
                                curves=copy.deepcopy(profiles), style=style, closed=False,
                                rebuild_point_count=count, sample_geometry=True,
                                inspect_inputs=True, inspect_rebuilt=True, allow_empty=True)
                            if start: op['start_point'] = [-.5, .25, -2.]
                            if end: op['end_point'] = [.5, -.25, 2. * sections]
                            operations.append(op)
    if commands:
        operations = [o for o in operations if o['rebuild_point_count'] >= 3
                      and not o.get('start_point') and not o.get('end_point')]
        for op in operations:
            op['id'] = op['id'].replace('loft-rebuild-', 'loft-rebuild-command-', 1)
            op['command'] = True
            op.pop('inspect_rebuilt', None)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--smoke', action='store_true')
    modes.add_argument('--bounds', action='store_true')
    modes.add_argument('--commands', action='store_true')
    args = parser.parse_args()
    family = 'loft_rebuild_smoke' if args.smoke else 'loft_rebuild'
    if args.bounds: family = 'loft_rebuild_bounds'
    if args.commands: family = 'loft_rebuild_commands'
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if output.exists():
        raise ValueError('successful native Loft captures are immutable')
    query = request(args.smoke, args.bounds, args.commands)
    fixture.write_text(json.dumps(query, indent=2) + '\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {p: digest(HERE / p) for p in SOURCES}
    print('Capturing {} Loft Rebuild cases'.format(len(query['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleLoftRebuild20261025').run_rhino(query, timeout=1200)
    if sources != {p: digest(HERE / p) for p in SOURCES}:
        raise ValueError('Loft Rebuild producer changed during capture')
    if [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]:
        raise ValueError('Loft Rebuild coverage differs')
    data['provenance'] = dict(request_sha256=digest(fixture), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
