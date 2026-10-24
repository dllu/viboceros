"""Qualify mesh outline geometry from measured contours and frozen native inputs."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from .mesh_outline_cases import FAMILIES, request
from .outline_comparison import EPSILON, boundary_error, contour, sampled_boundary_distance
from .qualification_sources import with_shared_sources

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PROJECTION_GAPS = ({'outline_context_far_{}_quad'.format(i) for i in range(8)}
                   | {'outline_facing_far_' + str(far) for far in range(40, 44)}
                   | {name for name in FAMILIES['facing'] if name.startswith('outline_facing_order_40_')})
PRECISION_GAPS = {'outline_scale_uniform_5'}
GAPS = PROJECTION_GAPS | PRECISION_GAPS
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
    'mesh_outline_cases.py', 'mesh_outline_scale_cases.py', 'mesh_outline_probe.py', 'generate_mesh_outline.py',
    'outline_comparison.py',
    'report_mesh_outline.py', 'test_mesh_outline.py', 'client.py', 'rhino_worker.py',
    'structural_sdk.py', 'merge_edges_probe.py', 'qualification_sources.py',
]])


def native():
    rows, qualified, versions, paths = {}, set(), set(), set(SOURCES)
    for family, cases in FAMILIES.items():
        qualified_family = family != 'sdk'
        family_name = 'mesh_outline_' + family
        query = HERE / 'fixtures' / (family_name + '.json')
        observation = HERE / 'observations' / (family_name + '.json')
        paths.update(str(p.relative_to(ROOT)) for p in [query, observation])
        if json.loads(query.read_text()) != request(family):
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
            if qualified_family or value['input']['method'] == 'plane':
                qualified.add(row['id'])
    if len(versions) != 1:
        raise ValueError('mixed outline native versions')
    return versions.pop(), rows, qualified, paths


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
        for path in row['outlines']:
            contour(path)
        deviation = sampled_boundary_distance(row['outlines'], references[name]['outlines'])
        try:
            measured = boundary_error(row['outlines'], references[name]['outlines'], maximum=None)
        except ValueError:
            measured = None
        if name in PROJECTION_GAPS:
            if (measured is not None or row['max_error'] is not None
                    or deviation is None or deviation <= EPSILON):
                raise ValueError('outline projection gap set differs')
        elif (measured is None or type(row['max_error']) not in (float, int)
              or not math.isfinite(row['max_error']) or abs(measured - row['max_error']) > 1e-14):
            raise ValueError('outline error declaration differs from measured contours')
        elif (measured > EPSILON) != (name in PRECISION_GAPS):
            raise ValueError('outline precision gap set differs')
        rows[name] = dict(row, max_error=measured,
                         maximum_sampled_boundary_deviation=deviation)
    if set(rows) != required:
        raise ValueError('measured outline case coverage differs')
    return dict(scope='mesh outline geometry; command lifecycle qualified separately',
                native_version=version, epsilon=EPSILON, native_workflows=len(references),
                replayed_geometry_workflows=len(rows),
                qualified_geometry_workflows=len(rows) - len(GAPS),
                projection_gaps=sorted(PROJECTION_GAPS),
                projection_precision_gaps=sorted(PRECISION_GAPS),
                exploratory_viewport_workflows_without_recorded_frustum=sorted(set(references) - required),
                max_absolute_error=max(row['max_error'] for name, row in rows.items() if name not in GAPS),
                contour_comparison='oriented cyclic vertices after removal of redundant collinear points; output order and start point may differ',
                pending=['curved surface render tessellation parity',
                         'frustum-dependent warped perspective quad outline parity',
                         'large-scale native projection precision parity',
                         'native eye-plane crossing and numerical range qualification'],
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
