"""Capture public RhinoCommon point-ended Loft surfaces without modifying inputs."""
import argparse
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
from .client import OracleClient

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_loft_points.py', 'fixtures/loft.json', 'client.py',
           'rhino_worker.py', 'structural_sdk.py']


def request(smoke=False, more_sections=False):
    existing = json.loads((HERE / 'fixtures/loft.json').read_text())
    circle = next(curve for op in existing['operations'] for curve in op['curves']
                  if curve['degree'] == 2 and len(curve['control_points']) == 9)
    line = dict(degree=1, knots=[0., 0., 1., 1.],
                control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [2., 0., 0.]]])
    quarter = dict(degree=2, knots=[0., 0., 0., 1., 1., 1.],
                   control_points=[dict(point=p, weight=w) for p, w in
                                   zip([[2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
                                       [1., math.sqrt(.5), 1.])])
    operations = []
    for shape, base in [('line', line), ('quarter', quarter), ('circle', circle)]:
        # The separate retained linear diagnostics include a native process
        # exit on three profiles; extend the rational-profile coverage here.
        if more_sections and shape == 'line':
            continue
        for count in ([3, 4] if more_sections else [1, 2]):
            curves = []
            for index in range(count):
                curve = copy.deepcopy(base)
                for cp in curve['control_points']:
                    x, y, _ = cp['point']
                    cp['point'] = [(1. + .25 * index) * x + .2 * index,
                                   (1. + .25 * index) * y, 1. + 2. * index]
                curves.append(curve)
            for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
                for start, end in [(True, False), (False, True), (True, True)]:
                    if smoke and not (shape == 'circle' and count == 1):
                        continue
                    op = dict(op='loft', id='loft-points-{}-{}-{}-{}{}'.format(
                        shape, count, style, int(start), int(end)), curves=curves,
                        style=style, closed=False, command=False, sample_geometry=True,
                        inspect_inputs=True, allow_empty=True)
                    if start:
                        op['start_point'] = [-.5, .25, -1.]
                    if end:
                        op['end_point'] = [.5, -.25, 3. + 2. * count if more_sections else 5.]
                    operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8),
                operations=operations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--smoke', action='store_true')
    modes.add_argument('--more-sections', action='store_true')
    args = parser.parse_args()
    family = 'loft_points_smoke' if args.smoke else 'loft_points'
    if args.more_sections:
        family = 'loft_points_more_sections'
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if output.exists():
        raise ValueError('successful native Loft captures are immutable')
    query = request(args.smoke, args.more_sections)
    fixture.write_text(json.dumps(query, indent=2) + '\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {p: digest(HERE / p) for p in SOURCES}
    print('Capturing {} point-ended Loft cases'.format(len(query['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleLoftPoints20261025').run_rhino(query, timeout=1200)
    if sources != {p: digest(HERE / p) for p in SOURCES}:
        raise ValueError('native point-ended Loft producer changed during capture')
    if [row['id'] for row in data['results']] != [op['id'] for op in query['operations']]:
        raise ValueError('native point-ended Loft coverage differs')
    data['provenance'] = dict(request_sha256=digest(fixture), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
