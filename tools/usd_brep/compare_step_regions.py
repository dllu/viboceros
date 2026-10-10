"""Qualify STEP material regions after native Rhino instance placement."""
import argparse
import json
import math
from pathlib import Path

CASES = ('sphere-cavity', 'concentric-shells', 'nested-island', 'torus-cavity',
         'torus-hole', 'box-two-cavities', 'cone-cavity')


def sphere(point, center, radius):
    return abs(math.sqrt(sum((a-b)**2 for a, b in zip(point, center)))-radius)


def boundary_error(case, point):
    x, y, z = point
    if not all(math.isfinite(v) for v in point):
        raise ValueError('nonfinite boundary sample')
    if case in ('concentric-shells', 'nested-island'):
        radii = (4., 2., .5) if case == 'nested-island' else (4., 2.)
        return min(sphere(point, (0., 0., 0.), r) for r in radii)
    if case.startswith('torus-'):
        torus = abs(math.hypot(math.hypot(x, y)-4., z)-1.)
        cavity = sphere(point, (4., 0., 0.), .25) if case == 'torus-cavity' else sphere(point, (0., 0., 0.), .5)
        return min(torus, cavity)
    if case == 'cone-cavity':
        if z < -1e-8 or z > 5.+1e-8 or math.hypot(x, y) > 2.+1e-8:
            raise ValueError('sample leaves cone bounds')
        side = abs(math.hypot(x, y)-2.*(1.-z/5.))/math.sqrt(1.16)
        return min(side, abs(z), sphere(point, (0., 0., 1.), .25))
    if any(v < -1e-8 or v > 10.+1e-8 for v in point):
        raise ValueError('sample leaves box bounds')
    box = min(min(abs(v), abs(v-10.)) for v in point)
    cavities = [(sphere(point, (5., 5., 5.), 2.))] if case == 'sphere-cavity' else [
        sphere(point, (3., 5., 5.), 1.), sphere(point, (7., 5., 5.), 1.)]
    return min([box] + cavities)


def expected_volumes(case):
    ball = lambda r: 4.*math.pi*r**3/3.
    torus = 8.*math.pi**2
    return {
        'sphere-cavity': [1000.-ball(2.)],
        'concentric-shells': [ball(4.)-ball(2.)],
        'nested-island': [ball(4.)-ball(2.), ball(.5)],
        'torus-cavity': [torus-ball(.25)],
        'torus-hole': [torus, ball(.5)],
        'box-two-cavities': [1000.-2.*ball(1.)],
        'cone-cavity': [20.*math.pi/3.-ball(.25)],
    }[case]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native', type=Path)
    parser.add_argument('properties', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--occt', type=Path, required=True)
    args = parser.parse_args()
    native = json.loads(args.native.read_text())
    source = {r['id']: r for r in json.loads(args.properties.read_text())}
    occt = {r['id']: r for r in json.loads(args.occt.read_text())}
    results = native['results']
    if len(results) != len(CASES) or {r['id'] for r in results} != set(CASES):
        raise ValueError('expected seven unique material fixtures')
    rows = []
    for record in results:
        case, value = record['id'], record['value']
        objects = value['objects']
        expected = sorted(expected_volumes(case))
        native_material = len(objects) == len(expected)
        shell_volumes = {
            'sphere-cavity': [1000., 32.*math.pi/3.],
            'concentric-shells': [256.*math.pi/3., 32.*math.pi/3.],
            'nested-island': [256.*math.pi/3., 32.*math.pi/3., math.pi/6.],
            'torus-cavity': [8.*math.pi**2, math.pi/48.],
            'torus-hole': expected,
            'box-two-cavities': [1000., 4.*math.pi/3., 4.*math.pi/3.],
            'cone-cavity': [20.*math.pi/3., math.pi/48.],
        }[case]
        native_expected = expected if native_material else sorted(shell_volumes)
        if not value['imported'] or len(objects) != len(native_expected):
            raise ValueError(case + ': unexpected native shell/region count')
        solids = occt[case]['solids']
        if len(solids) != len(expected) or any(not s['valid'] for s in solids):
            raise ValueError(case + ': independent STEP material grouping is invalid')
        occt_volumes = sorted(s['volume'] for s in solids)
        if any(not math.isfinite(v) for v in occt_volumes):
            raise ValueError('nonfinite independent material volume')
        occt_errors = [abs(a-b)/abs(b) for a, b in zip(occt_volumes, expected)]
        if max(occt_errors) > 1e-8:
            raise ValueError(case + ': independent STEP material volume changed')
        volumes, maximum, count = [], 0., 0
        for obj in objects:
            if not obj['valid'] or not obj['solid'] or obj['orientation'] != 'Outward':
                raise ValueError(case + ': invalid or incorrectly oriented material solid')
            volume = obj['volume']
            if volume is None or not math.isfinite(volume):
                raise ValueError('nonfinite material volume')
            volumes.append(volume)
            g = obj['geometry']
            samples = [p for e in g['edges'] for p in e['curve']['samples']]
            samples += [p for f in g['faces'] for loop in f['loops'] for t in loop for p in t['lifted']]
            for point in samples:
                error = boundary_error(case, point)
                if error > 1e-8:
                    raise ValueError(case + ': boundary geometry changed')
                maximum, count = max(maximum, error), count+1
        errors = [abs(a-b)/abs(b) for a, b in zip(sorted(volumes), native_expected)]
        if count == 0 or max(errors) > 5e-8:
            raise ValueError(case + ': material volume changed')
        source_error = abs(sum(occt_volumes)-source[case]['volume'])/abs(source[case]['volume'])
        if source_error > 1e-8:
            raise ValueError(case + ': aggregate source material volume changed')
        rows.append(dict(id=case, occt_material_solids=len(solids), occt_volumes=occt_volumes,
            maximum_occt_relative_volume_error=max(occt_errors), source_relative_volume_error=source_error,
            rhino_material_transfer=native_material, rhino_leaf_solids=len(objects),
            rhino_volumes=sorted(volumes), maximum_rhino_leaf_volume_error=max(errors),
            boundary_samples=count, maximum_boundary_error=maximum,
            top_level_types=value['top_level_types'], top_level_objects=value['top_level_objects']))
    args.output.write_text(json.dumps(dict(rhino_engine=native['engine_version'], occt_engine='7.6.3', cases=rows,
        geometric_epsilon=1e-8, occt_relative_volume_epsilon=1e-8, rhino_relative_leaf_volume_epsilon=5e-8,
        scope='OCCT validates material solids and volumes. Rhino boundary samples qualify shell geometry, '
              'but Rhino imports void regions as blocks of separate outward bodies and loses material semantics. '
              'General self-intersection validity and complete interactive equivalence remain unproven.'), indent=2)+'\n')
    print('OCCT qualified seven material cases and %d solids; Rhino sampled %d boundary points, with %d void-import limitations' % (
        sum(r['occt_material_solids'] for r in rows), sum(r['boundary_samples'] for r in rows),
        sum(not r['rhino_material_transfer'] for r in rows)))


if __name__ == '__main__':
    main()
