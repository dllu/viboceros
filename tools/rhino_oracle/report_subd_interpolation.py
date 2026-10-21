"""Qualify interpolated SubD faces, append and physical boundary drafts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subd_append import ROOT, HERE, SOURCES as BASE_SOURCES, check_objects

GROUPS = [('face_create_subd_interpolated', 21),
          ('face_create_subd_interpolated_drafts', 12),
          ('face_create_subd_interpolated_order', 6)]
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'tools/rhino_oracle/report_subd_interpolation.py',
    'tools/rhino_oracle/test_subd_interpolation.py',
] + [f'tools/rhino_oracle/{directory}/{name}.json'
     for name, _ in GROUPS for directory in ['fixtures', 'observations']]))


def summarize(text):
    native = []
    versions = set()
    for name, count in GROUPS:
        request = json.loads((HERE / 'fixtures' / (name + '.json')).read_text())
        response = json.loads((HERE / 'observations' / (name + '.json')).read_text())
        ids = [op['id'] for op in request['operations']]
        if len(ids) != count or len(set(ids)) != count or [r['id'] for r in response['results']] != ids:
            raise ValueError('incomplete native interpolation observations')
        versions.add(response['engine_version'])
        native.extend(response['results'])
    expected = {row['id']: row['value'] for row in native}
    if len(expected) != 39:
        raise ValueError('expected thirty-nine unique interpolation and ordering recipes')
    for key, value in expected.items():
        control = key in ['subd_interp_order_control', 'subd_interp_order_reverse_control',
                          'subd_interp_order_mesh_control']
        if value.get('success') is not True or value['input'].get('interpolated') is not (not control):
            raise ValueError('native interpolation command failed')
        if value['after_script'] != value['redo']:
            raise ValueError('native interpolation history differs')
        if any('surface_mesh' not in output['subd'] for output in value['after_script'] if 'subd' in output):
            raise ValueError('missing native interpolation surface')
    stages = {}
    maximum_control = maximum_surface = 0.0
    for stage in ['COMMAND', 'GETTER']:
        rows = [json.loads(match.group(1)) for match in re.finditer(
            r'^SUBD_INTERP_' + stage + r' (.+)$', text, re.MULTILINE)]
        ids = [key for key, value in expected.items()
               if stage == 'GETTER' or not value['input'].get('cancel')]
        if [row['id'] for row in rows] != ids:
            raise ValueError('missing, duplicate or reordered interpolation ' + stage)
        for row in rows:
            if row.get('grouped_history') is not True:
                raise ValueError('unqualified interpolation history')
            control, surface = check_objects(row['check'], expected[row['id']]['after_script'])
            maximum_control = max(maximum_control, control)
            maximum_surface = max(maximum_surface, surface)
        stages[stage.lower()] = rows
    return dict(
        model_epsilon=1e-6, recipes=39, interpolated_recipes=36, ordering_controls=3,
        command_cases=38, getter_cases=39, physical_input_recipes=18,
        native_engine_versions=sorted(versions),
        full_native_parity=False, performance_qualified=False,
        maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface, stages=stages,
        guarantees=[
            'Interpolated=Yes fits vertex limit stations to picked or InPlane-projected points. Original source limit stations remain the constraints for reused vertices; raw control coordinates determine joins.',
            'Triangle, quad, concave, twisted, reversed, n-gon and adjacent multiple-face controls match the captured native control nets, topology, tags and limit stations.',
            'Append coverage includes corner and crease sources, detached components, a shared-vertex fan, a later connecting bridge and an n-gon source.',
            'Twelve physical native input recipes cover boundary picks, both chain directions, folded strips, new boundaries, point Undo, triangles, n-gons, tilted InPlane constraints and double-Escape cancellation.',
            'Six further ordering controls isolate folded concave quads, reversed picks, direct entry and noninterpolated mesh/SubD output; both simple corner orders retain coherent source-boundary winding.',
            'Command and viewport getters match all captured density-two limit meshes by bidirectional facet sampling, source identity, attributes, selection and grouped Undo/Redo. Drafts leave source geometry and history unchanged until completion.',
            'A sparse Rust factorization fits all three coordinates using public OpenNURBS limit masks. Native vertex residuals are checked against the document absolute tolerance before applying a result; failure preserves the document.',
        ], limits=[
            'Surface comparison uses sampled facets, not a continuous Hausdorff certificate. Dynamic previews show draft boundaries; interpolation is solved at completion.',
            'General imported sharpness, arbitrary nonmanifold neighborhoods, coincident distinct source controls and texture/material channels require further native qualification.',
            'Mixing output families inside an existing draft is not implemented. Interpolated is remembered and affects only SubD output.',
            'Sparse masks are limited to four million coefficients and 32 million local basis work units. Factor fill and comparative performance are unqualified; singular or unresolved fits return an error.',
        ], sha256={path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in SOURCES})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(path.read_text() for path in args.logs))
    (ROOT / 'docs/subd-interpolation-qualification.json').write_text(
        json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
