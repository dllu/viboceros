"""Physical-geometry comparison for polyhedral Boolean API responses.

Face partitions and supporting parameterizations may differ. These are finite
boundary witnesses, not continuous Hausdorff bounds or interactive-command tests.
"""
import math
from .client import _validate_response


def _sub(a, b):
    return tuple(x-y for x, y in zip(a, b))


def _dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def _cross(a, b):
    return tuple(a[(i+1) % 3]*b[(i+2) % 3]-a[(i+2) % 3]*b[(i+1) % 3]
                 for i in range(3))


def _on_boundary(p, faces, epsilon):
    for face in faces:
        loops = face['loops']
        ring = loops[0]
        normals = (_cross(_sub(ring[i], ring[0]), _sub(ring[i+1], ring[0]))
                   for i in range(1, len(ring)-1))
        normal = next((n for n in normals if _dot(n, n) > 0), None)
        if normal is None or abs(_dot(normal, _sub(p, ring[0]))) > epsilon*math.sqrt(_dot(normal, normal)):
            continue
        axis = max(range(3), key=lambda i: abs(normal[i]))
        x, y = (axis+1) % 3, (axis+2) % 3
        inside = False
        for loop in loops:
            for i, a in enumerate(loop):
                b = loop[(i+1) % len(loop)]
                direction = _sub(b, a)
                length2 = _dot(direction, direction)
                t = 0 if length2 == 0 else max(0, min(1, _dot(_sub(p, a), direction)/length2))
                delta = _sub(p, tuple(a[j]+t*direction[j] for j in range(3)))
                if _dot(delta, delta) <= epsilon*epsilon:
                    return True
                if ((a[y] > p[y]) != (b[y] > p[y])
                        and p[x] < a[x]+(p[y]-a[y])*(b[x]-a[x])/(b[y]-a[y])):
                    inside = not inside
        if inside:
            return True
    return False


def _witnesses(body):
    yield from body['vertices']
    for edge in body['edge_samples']:
        yield from edge
    for face in body['faces']:
        # Mean vertices is safe only for convex, hole-free faces. Verify the
        # point in the source face before using it as an interior witness.
        ring = face['loops'][0]
        center = tuple(sum(p[i] for p in ring)/len(ring) for i in range(3))
        if _on_boundary(center, [face], 1e-12):
            yield center


def compare_polyhedral_responses(viboceros, rhino, mass_epsilon=1e-9, boundary_epsilon=1e-7):
    """Return per-recipe diagnostics for physical inputs and complete outputs.

    Native null/empty outcomes and invalid/open outputs stay explicit. A kernel
    rejection of a native open result is reported as a semantic difference.
    No timing or overall Rhino parity claim is made by this comparison.
    """
    if not all(type(e) in (int, float) and math.isfinite(e) and e > 0 for e in (mass_epsilon, boundary_epsilon)):
        raise ValueError('comparison epsilons must be finite and positive')
    _validate_response(viboceros, 'viboceros')
    _validate_response(rhino, 'rhino')
    if viboceros['iterations'] != rhino['iterations']:
        raise ValueError('polyhedral iteration counts must match')
    left = {r['id']: r['value'] for r in viboceros['results']}
    right = {r['id']: r['value'] for r in rhino['results']}
    if len(left) != len(viboceros['results']) or len(right) != len(rhino['results']) or left.keys() != right.keys():
        raise ValueError('polyhedral recipe ids must be unique and identical')
    rows = []
    for recipe, a in left.items():
        b = right[recipe]
        differences = []
        if a.get('kernel_error'):
            differences.append('kernel rejected boundary: '+a['kernel_error'])
        if any(not g['valid'] or not g['solid'] for g in b['outputs']):
            differences.append('native returned invalid or open geometry')
        if not differences:
            for label, own, native in [('inputs', a['inputs'], b['inputs']),
                                        ('outputs', a['outputs'], b['outputs'])]:
                if label == 'inputs' and len(own) != len(native):
                    differences.append('input count differs')
                    continue
                groups = list(zip(own, native)) if label == 'inputs' else [(own, native)]
                for index, (v, r) in enumerate(groups):
                    vs, rs = ([v], [r]) if label == 'inputs' else (v, r)
                    if any(not g['valid'] or not g['solid'] for g in vs+rs):
                        differences.append(f'{label}[{index}] is invalid or open')
                        continue
                    if bool(vs) != bool(rs):
                        differences.append(f'{label}[{index}] empty region differs')
                        continue
                    if not vs:
                        continue
                    volume_value = (lambda g: abs(g['volume'])) if label == 'inputs' else (lambda g: g['volume'])
                    if any(sum(volume_value(g) for g in bodies) == 0 for bodies in (vs, rs)):
                        differences.append(f'{label}[{index}] has zero aggregate signed volume')
                        continue
                    for metric in ('volume', 'area'):
                        value = volume_value if metric == 'volume' else (lambda g: g['area'])
                        if abs(sum(value(g) for g in vs)-sum(value(g) for g in rs)) > mass_epsilon:
                            differences.append(f'{label}[{index}] {metric} differs')
                    for axis in range(3):
                        for side, reduce in ((0, min), (1, max)):
                            if abs(reduce(g['bounds'][side][axis] for g in vs)-reduce(g['bounds'][side][axis] for g in rs)) > mass_epsilon:
                                differences.append(f'{label}[{index}] bounds differ')
                        centroids = [sum(volume_value(g)*g['centroid'][axis] for g in bodies)/sum(volume_value(g) for g in bodies)
                                     for bodies in (vs, rs)]
                        if abs(centroids[0]-centroids[1]) > mass_epsilon:
                            differences.append(f'{label}[{index}] centroid differs')
                    for bodies, counterparts in ((vs, rs), (rs, vs)):
                        faces = [f for g in counterparts for f in g['faces']]
                        if any(not _on_boundary(p, faces, boundary_epsilon)
                               for g in bodies for p in _witnesses(g)):
                            differences.append(f'{label}[{index}] boundary witness differs')
            if a['returned_null'] != b['returned_null']:
                differences.append('native null response differs from mathematical set result')
        rows.append(dict(id=recipe, passed=not differences, differences=differences))
    return rows
