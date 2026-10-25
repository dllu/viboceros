"""Freeze public Loft Refit outputs beside independent Curve.Fit definitions."""
import argparse
import copy
import json
from pathlib import Path
from .capture_family import capture
from .generate_loft_rebuild import request as rebuilt_request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_loft_refit.py', 'generate_loft_rebuild.py', 'fixtures/loft.json',
           'rhino_worker.py', 'client.py', 'structural_sdk.py']


def request(smoke=False):
    originals = rebuilt_request()['operations']
    operations = []
    shapes = ['line', 'quarter', 'cubic', 'circle'] if smoke else ['line', 'quarter', 'cubic', 'circle', 'polygon']
    for shape in shapes:
        source = next(o for o in originals if o['id'] == 'loft-rebuild-{}-0-normal-0'.format(shape))
        for tolerance in ([0., .001] if smoke else [0., 1e-6, .001, .1]):
            for style in (['normal', 'straight'] if smoke else ['normal', 'loose', 'tight', 'straight', 'uniform']):
                op = copy.deepcopy(source)
                op.pop('rebuild_point_count'); op.pop('inspect_rebuilt', None)
                op.update(id='loft-refit-{}-{}-{}'.format(shape, tolerance, style),
                          style=style, refit_tolerance=tolerance, inspect_refitted=tolerance > 0.)
                operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--smoke', action='store_true')
    args = parser.parse_args()
    family = 'loft_refit_smoke' if args.smoke else 'loft_refit'
    capture(family, request(args.smoke), SOURCES,
            'VibocerosOracleLoftRefit20261025')


if __name__ == '__main__':
    main()
