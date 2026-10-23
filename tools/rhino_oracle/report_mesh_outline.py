"""Qualify mesh outline geometry from measured contours and frozen native inputs."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from .mesh_outline_cases import SDK, PROJECTIONS, request
from .qualification_sources import with_shared_sources

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EPSILON = 1e-7
FAMILIES = {'mesh_outline_sdk': (False, SDK), 'mesh_outline_projections': (True, PROJECTIONS)}
SOURCES = with_shared_sources([
    'Cargo.toml', 'Cargo.lock', 'crates/viboceros-geometry/Cargo.toml',
    'crates/viboceros-geometry/src/lib.rs', 'crates/viboceros-geometry/src/error.rs',
    'crates/viboceros-geometry/src/mesh.rs', 'crates/viboceros-geometry/src/mesh/outline.rs',
    'crates/viboceros-geometry/src/frame.rs', 'crates/viboceros-geometry/src/point.rs',
    'crates/viboceros-geometry/src/vector.rs', 'crates/viboceros-geometry/src/polyline.rs',
    'crates/viboceros-geometry/src/transform/orientation.rs',
    'crates/viboceros-geometry/src/binary_accumulator.rs',
    'crates/viboceros-geometry/tests/mesh_outline.rs', 'third_party/README.md',
    'docs/mesh-outline-geometry.md',
] + ['tools/rhino_oracle/' + name for name in [
    'mesh_outline_cases.py', 'mesh_outline_probe.py', 'generate_mesh_outline.py',
    'report_mesh_outline.py', 'test_mesh_outline.py', 'client.py', 'rhino_worker.py',
    'structural_sdk.py', 'merge_edges_probe.py', 'qualification_sources.py',
]])


def native():
    rows, qualified, versions, paths = {}, set(), set(), set(SOURCES)
    for family, (projections, cases) in FAMILIES.items():
        query = HERE / 'fixtures' / (family + '.json')
        observation = HERE / 'observations' / (family + '.json')
        paths.update(str(p.relative_to(ROOT)) for p in [query, observation])
        if json.loads(query.read_text()) != request(projections=projections):
            raise ValueError('fixed outline request changed')
        data = json.loads(observation.read_text())
        if data['provenance']['request_sha256'] != hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('outline request hash differs')
        for digest in data['provenance']['sources'].values():
            path = HERE / 'observations/source_snapshots' / (digest + '.gz')
            if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest() != digest:
                raise ValueError('outline frozen producer hash differs')
            paths.add(str(path.relative_to(ROOT)))
        versions.add(data['engine_version'])
        if [row['id'] for row in data['results']] != list(cases):
            raise ValueError('native outline case coverage differs')
        for row in data['results']:
            value = row['value']
            if value['input'] != cases[row['id']] or value['source_unchanged'] is not True:
                raise ValueError('native outline input or source preservation differs')
            if value['returned_null'] is not (not value['outlines']):
                raise ValueError('native empty outline contract differs')
            rows[row['id']] = value
            if projections or value['input']['method'] == 'plane':
                qualified.add(row['id'])
    if len(versions) != 1:
        raise ValueError('mixed outline native versions')
    return versions.pop(), rows, qualified, paths


def contour(value):
    if (not isinstance(value, list) or len(value) < 4
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (float, int) or not math.isfinite(x) for x in p) for p in value)
            or math.dist(value[0], value[-1]) > EPSILON):
        raise ValueError('finite closed outline contour required')
    points = value[:-1]
    while len(points) > 3:
        for i, b in enumerate(points):
            a, c = points[i - 1], points[(i + 1) % len(points)]
            ac = [y - x for x, y in zip(a, c)]
            length = sum(x*x for x in ac)
            t = sum((y - x)*z for x, y, z in zip(a, b, ac)) / length if length else -1.
            if 0. <= t <= 1. and math.dist(b, [x + t*d for x, d in zip(a, ac)]) <= EPSILON:
                points = points[:i] + points[i + 1:]
                break
        else:
            break
    return points


def boundary_error(actual, expected):
    if not isinstance(actual, list) or len(actual) != len(expected):
        raise ValueError('outline contour count differs')
    expected = [contour(p) for p in expected]
    maximum = 0.
    for path in actual:
        points = contour(path)
        choices = [(max(math.dist(a, native[(start + i) % len(native)]) for i, a in enumerate(points)), index)
                   for index, native in enumerate(expected) if len(native) == len(points)
                   for start in range(len(native))]
        if not choices or min(choices)[0] > EPSILON:
            raise ValueError('oriented outline boundary differs')
        distance, index = min(choices)
        maximum = max(maximum, distance)
        expected.pop(index)
    return maximum


def summarize(text):
    version, references, required, paths = native()
    rows = {}
    for line in text.splitlines():
        if 'MESH_OUTLINE_SDK ' not in line:
            continue
        row = json.loads(line.split('MESH_OUTLINE_SDK ', 1)[1])
        name = row['case']
        if name not in required or name in rows:
            raise ValueError('duplicate or unsupported measured outline case')
        if row['source_unchanged'] is not True:
            raise ValueError('local outline query changed its source')
        measured = boundary_error(row['outlines'], references[name]['outlines'])
        if (type(row['max_error']) not in (float, int) or not math.isfinite(row['max_error'])
                or abs(measured - row['max_error']) > 1e-14):
            raise ValueError('outline error declaration differs from measured contours')
        rows[name] = dict(row, max_error=measured)
    if set(rows) != required:
        raise ValueError('measured outline case coverage differs')
    return dict(scope='mesh outline geometry; command and application integration pending',
                native_version=version, epsilon=EPSILON, native_workflows=len(references),
                qualified_geometry_workflows=len(rows),
                exploratory_viewport_workflows_without_recorded_frustum=sorted(set(references) - required),
                max_absolute_error=max(row['max_error'] for row in rows.values()),
                contour_comparison='oriented cyclic vertices after removal of redundant collinear points; output order and start point may differ',
                pending=['MeshOutline command lifecycle and object selection', 'surface render meshes',
                         'application viewport integration', 'native eye-plane crossing and numerical range qualification'],
                geometry=[rows[name] for name in sorted(rows)],
                sha256={name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/mesh-outline-qualification.json')
    args = parser.parse_args()
    result = summarize('\n'.join(path.read_text() for path in args.logs))
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
    print('Qualified {} mesh outline geometry workflows'.format(result['qualified_geometry_workflows']))


if __name__ == '__main__':
    main()
