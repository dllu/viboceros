"""Compare measured polygon Boolean surfaces with immutable Rhino SDK results."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from .mesh_boolean_cases import SDK, CONTROLS, request
from .mesh_boolean_comparison import measures, sampled_boundary_distance, triangles
from .qualification_sources import with_shared_sources

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EPSILON = 1e-7
GAPS = ({'identical_'+op for op in ['union', 'intersection', 'difference']}
        | {'face_touch_intersection', 'face_touch_difference'}
        | {name for name in CONTROLS if name != 'coincident_face_touch_union'})
SOURCES = with_shared_sources([
    'Cargo.lock', 'crates/viboceros-smlib/src/lib.rs', 'crates/viboceros-smlib/src/native.rs',
    'crates/viboceros-smlib/src/native/mesh_boolean.rs', 'crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-smlib/native/bridge.h', 'crates/viboceros-smlib/native/mesh_boolean.h',
    'crates/viboceros-smlib/native/CMakeLists.txt', 'crates/viboceros-smlib/build.rs',
    'crates/viboceros-smlib/tests/mesh_boolean.rs', 'crates/viboceros-smlib/tests/support/mesh_boolean.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'third_party/usd-brep/source/SMLib/inc/SmPoly.h',
    'third_party/usd-brep/source/SMLib/inc/SmPolyMerge.h',
    'third_party/usd-brep/source/SMLib/inc/SmPolyIntersector.h',
    'third_party/usd-brep/source/SMLib/src/SmPoly.cpp',
    'third_party/usd-brep/source/SMLib/src/SmPolyMerge.cpp',
    'third_party/usd-brep/source/SMLib/src/SmPolyIntersector.cpp',
    'docs/mesh-boolean-geometry.md',
] + ['tools/rhino_oracle/'+name for name in [
    'mesh_boolean_cases.py', 'mesh_boolean_probe.py', 'generate_mesh_boolean.py',
    'mesh_boolean_comparison.py', 'report_mesh_boolean.py', 'test_mesh_boolean.py',
    'client.py', 'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py']])


def native():
    rows, versions, paths = {}, set(), set(SOURCES)
    for controls, cases in [(False, SDK), (True, CONTROLS)]:
        family = 'mesh_boolean_controls' if controls else 'mesh_boolean_sdk'
        query, capture = [HERE/folder/(family+'.json') for folder in ['fixtures', 'observations']]
        paths.update(str(p.relative_to(ROOT)) for p in [query, capture])
        if json.loads(query.read_text()) != request(controls=controls):
            raise ValueError('fixed mesh Boolean request changed')
        data = json.loads(capture.read_text())
        if data['provenance']['request_sha256'] != hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('mesh Boolean request digest differs')
        for sha in data['provenance']['sources'].values():
            path = HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest() != sha:
                raise ValueError('mesh Boolean frozen producer differs')
            paths.add(str(path.relative_to(ROOT)))
        versions.add(data['engine_version'])
        if [r['id'] for r in data['results']] != list(cases):
            raise ValueError('native mesh Boolean coverage differs')
        for row in data['results']:
            v = row['value']
            if v['input'] != cases[row['id']] or v['source_unchanged'] is not True:
                raise ValueError('native mesh Boolean input or preservation differs')
            if controls and not v['source_meshes_before'] == v['source_meshes_after'] == v['input']['meshes']:
                raise ValueError('mesh Boolean source geometry differs')
            list(triangles(v['meshes']))
            rows[row['id']] = v
    if len(versions) != 1:
        raise ValueError('mixed mesh Boolean native versions')
    return versions.pop(), rows, paths


def summarize(text):
    version, expected, paths = native()
    rows, gaps, maximum = {}, set(), 0.
    for line in text.splitlines():
        if 'MESH_BOOLEAN_SDK ' not in line:
            continue
        row = json.loads(line.split('MESH_BOOLEAN_SDK ', 1)[1])
        name = row['case']
        if name in rows or name not in expected:
            raise ValueError('duplicate or unsupported mesh Boolean measurement')
        if row['kernel_error'] is not None or row['source_unchanged'] is not True:
            raise ValueError('mesh Boolean kernel or preservation failure')
        area, volume = measures(row['meshes'])
        for field, computed in [('area', area), ('signed_volume', volume)]:
            if (type(row[field]) not in (float, int) or not math.isfinite(row[field])
                    or abs(row[field]-computed) > 1e-10):
                raise ValueError('mesh Boolean measures differ from output geometry')
        reference = expected[name]
        distance = sampled_boundary_distance(row['meshes'], reference['meshes'])
        declared = row['maximum_sampled_boundary_deviation']
        if ((distance is None) != (declared is None) or (distance is not None and (
                type(declared) not in (float, int) or not math.isfinite(declared)
                or abs(distance-declared) > 1e-10))):
            raise ValueError('mesh Boolean sampled boundary declaration differs')
        matched = distance is not None and distance <= EPSILON
        matched &= abs(area-sum(reference['areas'])) <= EPSILON
        if all(v is not None for v in reference['volumes']):
            matched &= abs(volume-sum(reference['volumes'])) <= EPSILON
        if 'geometry_matches' in row and row['geometry_matches'] is not matched:
            raise ValueError('mesh Boolean match label differs from measured geometry')
        if not matched:
            gaps.add(name)
        else:
            maximum = max(maximum, distance)
        rows[name] = dict(row, geometry_matches=matched, maximum_sampled_boundary_deviation=distance)
    if set(rows) != set(expected) or gaps != GAPS:
        raise ValueError('mesh Boolean measurement coverage or gap set differs')
    return dict(scope='native polygon Boolean backend; command/getter evidence is qualified separately',
                native_version=version, epsilon=EPSILON, replayed=len(rows),
                matched=len(rows)-len(gaps), compatibility_gaps=sorted(gaps),
                maximum_qualified_sampled_deviation=maximum,
                comparison='area, oriented volume and bidirectional surface samples at triangle vertices, edge midpoints and centroids; face tessellation may differ',
                pending=['coincident and face-contact native parity',
                         'curved surface meshing, extreme scales and broader native mesh coverage'],
                geometry=[rows[name] for name in sorted(rows)],
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-boolean-qualification.json')
    args = parser.parse_args()
    result = summarize(args.log.read_text())
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    print('Matched {} of {} mesh Boolean SDK cases'.format(result['matched'], result['replayed']))


if __name__ == '__main__':
    main()
