"""Qualify standalone SubD face creation against immutable Rhino observations."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
GROUPS = ['face_create_subd', 'face_create_subd_surface']
SOURCES = [
    'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h',
    'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/native/CMakeLists.txt',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.h',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/src/lib.rs',
    'crates/viboceros-geometry/src/error.rs',
    'crates/viboceros-geometry/src/point.rs',
    'crates/viboceros-geometry/src/transform.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/mesh/face_create.rs',
    'crates/viboceros-geometry/src/mesh/face_records.rs',
    'crates/viboceros-geometry/src/mesh/mass_triangles.rs',
    'crates/viboceros-geometry/src/mesh/closest.rs',
    'crates/viboceros-geometry/src/mesh/projection.rs',
    'crates/viboceros-document/src/geometry.rs',
    'crates/viboceros-document/src/control_points.rs',
    'crates/viboceros-document/src/control_points/geometry.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/face_create.rs',
    'crates/viboceros-command/src/interchange.rs',
    'crates/viboceros-command/tests/subd_native.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-io/src/three_dm.rs',
    'crates/viboceros-io/src/three_dm_units.rs',
    'crates/viboceros-io/src/three_dm/blocks.rs',
    'src/app.rs', 'src/app/face_create.rs', 'src/app/face_create/tests.rs',
    'src/viewport/display_cache.rs', 'src/viewport/selection.rs',
    'src/viewport/tests/object_selection.rs',
    'tools/rhino_oracle/face_create_probe.py',
    'tools/rhino_oracle/generate_face_create.py',
    'tools/rhino_oracle/report_subd.py', 'tools/rhino_oracle/test_subd.py',
] + [f'tools/rhino_oracle/{directory}/{name}.json'
     for name in GROUPS for directory in ['fixtures', 'observations']]


def summarize(text):
    native = []
    for name in GROUPS:
        response = json.loads((HERE / 'observations' / (name + '.json')).read_text())
        request = json.loads((HERE / 'fixtures' / (name + '.json')).read_text())
        if [row['id'] for row in response['results']] != [op['id'] for op in request['operations']]:
            raise ValueError('incomplete native SubD capture')
        native.extend(response['results'])
    expected = {row['id']: row['value']['after_script'][0]['subd'] for row in native}
    if len(native) != 20 or len(expected) != 20:
        raise ValueError('expected twenty unique captured SubD cases')
    stages = {}
    maximum_control = maximum_surface = 0.0
    for stage in ['COMMAND', 'GETTER']:
        rows = [json.loads(match.group(1)) for match in re.finditer(
            r'^SUBD_' + stage + r' (.+)$', text, re.MULTILINE)]
        if [row['id'] for row in rows] != list(expected):
            raise ValueError('missing, duplicate or reordered SubD results: ' + stage)
        for row in rows:
            check = row['check']
            target = expected[row['id']]
            if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get('attributes_and_selection') is not True:
                raise ValueError('unqualified SubD history, attributes or geometry')
            for key, reference in [('vertices', target['points']), ('faces', target['faces']), ('edges', target['edges'])]:
                if type(check.get(key)) is not int or check[key] != len(reference):
                    raise ValueError('SubD topology count differs')
            for key in ['maximum_control_or_limit_point_error', 'maximum_sampled_surface_error']:
                value = check.get(key)
                if type(value) not in (int, float) or not math.isfinite(value) or not 0 <= value <= 1e-6:
                    raise ValueError('invalid or excessive SubD error')
            if check.get('surface_sampled') is not ('surface_mesh' in target):
                raise ValueError('missing native limit-surface comparison')
            maximum_control = max(maximum_control, check['maximum_control_or_limit_point_error'])
            maximum_surface = max(maximum_surface, check['maximum_sampled_surface_error'])
        stages[stage.lower()] = rows
    return dict(
        model_epsilon=1e-6, recipes=10, captured_cases=20, surface_mesh_cases=10,
        full_native_parity=False, performance_qualified=False,
        maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface,
        opennurbs_commit=subprocess.check_output(
            ['git', '-C', str(ROOT / 'third_party/opennurbs'), 'rev-parse', 'HEAD'], text=True).strip(),
        stages=stages,
        guarantees=[
            'Standalone noninterpolated triangle, quad, concave, twisted and n-gon faces, reverse winding, InPlane and adjacent multiple faces match captured control topology and tags.',
            'Every control-vertex limit station matches Rhino. Ten density-two limit meshes pass bidirectional facet sampling at model epsilon.',
            'Command and interactive getters preserve native current-layer attributes, output selection and grouped Undo/Redo.',
            'Public OpenNURBS owns subdivision evaluation and the full SubD archive. SubD geometry survives 3DM Save/Open, affine transforms and control-point edits.',
        ], limits=[
            'Appending to existing SubDs and Interpolated=Yes are not implemented.',
            'Display tessellation samples exact limit points; linear facets between samples do not carry a Hausdorff or model-tolerance certificate.',
            'Certified tight bounds and exact SubD projection, intersections, border extraction and surface snaps are not implemented.',
            'Imported tags and sharpness remain in the archive; editing those properties, nonmanifold construction, texture/material channels and comparative speed remain unqualified.',
            'Construction permits 100000 vertices and faces, 400000 corners and a 64 MiB archive. Tessellation accepts density 1..6 with a one-million-quad budget.',
        ], sha256={path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in SOURCES})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(path.read_text() for path in args.logs))
    (ROOT / 'docs/subd-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
