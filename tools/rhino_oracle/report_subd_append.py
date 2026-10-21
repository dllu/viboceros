"""Qualify archival SubD face append, components, getters and grouped history."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
from .report_subd import ROOT, HERE, SOURCES as BASE_SOURCES

GROUPS=[('face_create_subd_append',12),('face_create_subd_append_extras',10),('face_create_subd_drafts',10)]
NORMAL_GROUP=('face_create_subd_normals',7)
SOURCES=list(dict.fromkeys(BASE_SOURCES+[
    'crates/viboceros-command/tests/subd_append_native.rs',
    'crates/viboceros-command/tests/support/subd_append.rs',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/structural_sdk.py',
    'tools/rhino_oracle/face_draft_input.py','tools/rhino_oracle/group_picking.py',
    'tools/rhino_oracle/report_subd_append.py','tools/rhino_oracle/test_subd_append.py',
]+[f'tools/rhino_oracle/{directory}/{name}.json'for name,_ in GROUPS+[NORMAL_GROUP] for directory in ['fixtures','observations']]))


def finite_error(value):
    if type(value) not in (int,float) or not math.isfinite(value) or not 0<=value<=1e-6:
        raise ValueError('invalid or excessive SubD append error')
    return value


def check_objects(check,targets):
    if check.get('qualified')is not True or check.get('attributes_selection_and_identity')is not True:
        raise ValueError('unqualified object attributes or geometry')
    maximum_control=maximum_surface=0.0
    if len(check['objects'])!=len(targets):raise ValueError('missing detached or source output')
    for actual,target in zip(check['objects'],targets):
        if actual.get('qualified')is not True:raise ValueError('unqualified object')
        if 'subd'in target:
            target=target['subd']
            if target.get('valid')is not True:raise ValueError('invalid native SubD')
            for key,values in [('vertices',target['points']),('edges',target['edges']),('faces',target['faces'])]:
                if type(actual.get(key))is not int or actual[key]!=len(values):raise ValueError('append control topology differs')
            if actual.get('surface_sampled')is not ('surface_mesh'in target):raise ValueError('missing limit-surface comparison')
            maximum_control=max(maximum_control,finite_error(actual.get('maximum_control_or_limit_point_error')))
        else:
            if actual.get('mesh')is not True:raise ValueError('output family differs')
            for key,values in [('vertices',target['mesh']['vertices']),('faces',target['mesh']['faces'])]:
                if type(actual.get(key))is not int or actual[key]!=len(values):raise ValueError('mesh output topology differs')
        maximum_surface=max(maximum_surface,finite_error(actual.get('maximum_sampled_surface_error')))
    return maximum_control,maximum_surface


def summarize(text):
    native=[];versions=set()
    for name,count in GROUPS:
        request=json.loads((HERE/'fixtures'/(name+'.json')).read_text())
        response=json.loads((HERE/'observations'/(name+'.json')).read_text())
        if len(response['results'])!=count or [row['id']for row in response['results']]!=[op['id']for op in request['operations']]:
            raise ValueError('missing native append observations')
        versions.add(response['engine_version']);native.extend(response['results'])
    expected={row['id']:row['value']for row in native}
    if len(expected)!=32:raise ValueError('expected thirty-two unique native cases')
    maximum_control=maximum_surface=0.0;stages={}
    for stage in ['COMMAND','GETTER']:
        rows=[json.loads(m.group(1))for m in re.finditer(r'^SUBD_APPEND_'+stage+r' (.+)$',text,re.MULTILINE)]
        ids=[key for key,value in expected.items()if stage=='GETTER' or not value['input'].get('cancel')]
        if [row['id']for row in rows]!=ids:raise ValueError('missing, duplicate or reordered append '+stage)
        for row in rows:
            check=row['check'];targets=expected[row['id']]['after_script']
            if row.get('grouped_history')is not True or check.get('qualified')is not True or check.get('attributes_selection_and_identity')is not True:
                raise ValueError('append geometry, attributes or history is unqualified')
            control,surface=check_objects(check,targets)
            maximum_control=max(maximum_control,control);maximum_surface=max(maximum_surface,surface)
        stages[stage.lower()]=rows
    name,count=NORMAL_GROUP
    normal_request=json.loads((HERE/'fixtures'/(name+'.json')).read_text())
    normal_response=json.loads((HERE/'observations'/(name+'.json')).read_text())
    versions.add(normal_response['engine_version'])
    normal_native=normal_response['results']
    if len(normal_native)!=count or [r['id']for r in normal_native]!=[o['id']for o in normal_request['operations']]:
        raise ValueError('missing native normal observations')
    normal_rows=[json.loads(m.group(1))for m in re.finditer(r'^SUBD_NORMAL_COMMAND (.+)$',text,re.MULTILINE)]
    if [r['id']for r in normal_rows]!=[r['id']for r in normal_native]:raise ValueError('missing, duplicate or reordered normal results')
    maximum_normal=0.0
    for row,target in zip(normal_rows,normal_native):
        control,surface=check_objects(row['geometry'],target['value']['after_script'])
        maximum_control=max(maximum_control,control);maximum_surface=max(maximum_surface,surface)
        check=row['normals'];corners=sum(4*len(o['subd']['surface_mesh']['faces'])for o in target['value']['after_script'])
        if check.get('qualified')is not True or type(check.get('corners'))is not int or check['corners']!=corners:
            raise ValueError('missing normal sectors')
        maximum_normal=max(maximum_normal,finite_error(check.get('maximum_corner_normal_error')))
    return dict(model_epsilon=1e-6,normal_epsilon=1e-6,recipes=32,command_cases=31,getter_cases=32,
        normal_cases=count,maximum_corner_normal_error=maximum_normal,normal_stages={'command':normal_rows},
        native_engine_versions=sorted(versions),full_native_parity=False,performance_qualified=False,
        maximum_control_or_limit_point_error=maximum_control,maximum_sampled_surface_error=maximum_surface,
        stages=stages,
        guarantees=[
            'Appending edits a decoded copy of the original SubD archive; the source object keeps identity, name and layer. Exact coincident vertices join, including the captured shared-vertex and three-face-edge nonmanifold cases.',
            'Joined boundary edges become smooth and their endpoint tags are recomputed. Unrelated source creases and corners remain unchanged in the captured controls.',
            'Whole new face sets are oriented against source boundaries and each other, including a component connected by a later bridge. Detached components become separate current-layer objects.',
            'Explicit Output switches create the requested family alongside an unchanged source of another family. Preselected, explicit-ID and postselected targets infer their family when Output is omitted.',
            'Physical native boundary picks, both chain directions, folded strips, new draft boundaries, triangles, n-gons, point Undo and double-Escape cancellation are replayed through command or viewport getters.',
            'Control topology, winding, tags, limit stations, sampled density-two surface meshes, selection, source identity and grouped history match every captured result at model epsilon.',
            'Seven additional native density-two captures compare exact sector normals, including shallow creases and a nonmanifold edge. The viewport consumes those normals without averaging across SubD sectors.',
            'Interactive drafts leave document geometry and history untouched until completion. One transaction applies source replacement and detached outputs; cancellation keeps the source geometry.',
        ],limits=[
            'Interpolated=Yes remains unsupported. Output may be changed before the first draft corner; mixing output families inside an existing face draft is not implemented.',
            'The original nonmanifold interior-edge control has no native density-two mesh; a separate normal capture supplies mesh and sector-normal comparisons for that construction. Arbitrary nonmanifold fans and coincident distinct source controls require further qualification.',
            'Original component data are preserved through the native append path; material/texture edits and crease-sharpness parity are not separately qualified.',
            'Display meshes sample exact limit points, but their linear facets have no model-tolerance certificate. Certified tight bounds, exact SubD projection/intersection, surface snaps and comparative speed remain unqualified.',
            'Construction retains the existing vertex, face, corner and archive budgets. Component adjacency uses a linear incidence star rather than a clique at shared vertices.',
        ],sha256={path:hashlib.sha256((ROOT/path).read_bytes()).hexdigest()for path in SOURCES})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    report=summarize('\n'.join(path.read_text()for path in args.logs))
    (ROOT/'docs/subd-append-qualification.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
