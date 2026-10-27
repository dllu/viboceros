"""Measure public deviation on independent endpoint-frame fitting candidates."""
import bisect
import copy
import json
import math
from .capture_family import HERE, capture


def frames(points, start_tangent, end_tangent):
    start, end = points[0], points[-1]
    chord = [b - a for a, b in zip(start, end)]
    length = math.hypot(*chord)
    axis = [v / length for v in chord]
    handles = []
    for tangent in [start_tangent, end_tangent]:
        norm = math.hypot(*tangent)
        direction = [v / norm for v in tangent]
        cosine = sum(a*b for a, b in zip(axis, direction))
        size = length * (2. - cosine*cosine) / 3.
        handles.append([v*size for v in direction])
    controls = [start, [a+h for a, h in zip(start, handles[0])],
                [b-h for b, h in zip(end, handles[1])], end]
    return dict(type='nurbs', degree=3, knots=[0.]*4 + [1.]*4,
                control_points=[dict(point=p, weight=1.) for p in controls])


def trim(curve, a, b):
    points = [c['point'] for c in curve['control_points']]
    positions = curve['knots'][1:-1]
    lengths = [0.]
    for first, last in zip(points, points[1:]):
        lengths.append(lengths[-1] + math.dist(first, last))
    def at(f):
        s = f * lengths[-1]
        i = max(0, min(bisect.bisect_right(lengths, s)-1, len(points)-2))
        x = (s-lengths[i])/(lengths[i+1]-lengths[i])
        return ([p+x*(q-p) for p, q in zip(points[i], points[i+1])],
                positions[i]+x*(positions[i+1]-positions[i]),
                [q-p for p, q in zip(points[i], points[i+1])])
    first, ta, u = at(a)
    last, tb, v = at(b)
    inside = [i for i in range(1, len(points)-1) if a*lengths[-1] < lengths[i] < b*lengths[-1]]
    clipped = dict(type='nurbs', degree=1,
        knots=[ta, ta]+[positions[i] for i in inside]+[tb, tb],
        control_points=[dict(point=p, weight=1.) for p in [first]+[points[i] for i in inside]+[last]])
    return clipped, frames([first, last], u, v)


def request():
    old = json.loads((HERE/'fixtures/loft_refit_adaptive_parameter_clocks.json').read_text())
    operations = []
    spans = [[0., 1.], [0., .5], [.25, .5], [.25, .375], [.3125, .375], [.3125, .34375]]
    for op in old['operations']:
        if op['refit_tolerance'] != 2.5e-7:
            continue
        reverse = '-clock-1-' in op['id']
        clock = op['curves'][0]['knots'][2]
        for index, bounds in enumerate(spans):
            a, b = [1.-bounds[1], 1.-bounds[0]] if reverse else bounds
            source, candidate = trim(op['curves'][0], a, b)
            operations.append(dict(op='curve_deviation',
                id='loft-refit-deviation-{}-{}-{}'.format(int(reverse), clock, index),
                curves=[source, candidate], postselect=False))
    hidden = json.loads((HERE/'fixtures/loft_refit_hidden_features.json').read_text())
    for op in hidden['operations']:
        if '-1.0-0-' not in op['id']:
            continue
        source = copy.deepcopy(op['curves'][0]);source['type']='nurbs'
        points = [c['point'] for c in source['control_points']]
        candidate = frames([points[0], points[-1]],
            [b-a for a, b in zip(points[0], points[1])],
            [b-a for a, b in zip(points[-2], points[-1])])
        operations.append(dict(op='curve_deviation', id=op['id'].replace('hidden-', 'deviation-hidden-'),
                               curves=[source, candidate], postselect=False))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-10, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_deviation', request(),
        ['generate_loft_refit_deviation.py', 'fixtures/loft_refit_adaptive_parameter_clocks.json',
         'fixtures/loft_refit_hidden_features.json', 'curve_deviation_probe.py', 'geometry_validity_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitDeviation20261027')
