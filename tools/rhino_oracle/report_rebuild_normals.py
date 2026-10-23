"""Qualify public normal rebuilding, compaction and independent command state."""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from .rebuild_normals_cases import SDK,COMMANDS,EXTRA_SDK,EXTRA_COMMANDS,request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE,ROOT,error
FAMILIES={'rebuild_normals_sdk':15,'rebuild_normals_command':18,'rebuild_normals_holdouts':8}
SOURCES=with_shared_sources([
    'Cargo.lock','src/app.rs','src/app/object_selection.rs','src/app/object_selection/rebuild_normals_tests.rs',
    'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/mesh_edit/mod.rs',
    'crates/viboceros-command/src/mesh_edit/rebuild_normals.rs','crates/viboceros-command/src/mesh_edit/rebuild_normals/tests.rs',
    'crates/viboceros-command/src/mesh_edit/replacement.rs','crates/viboceros-command/src/mesh_edit/triangulate.rs',
    'crates/viboceros-command/src/tests/document/registry.rs',
    'crates/viboceros-command/tests/rebuild_normals_native.rs','crates/viboceros-command/tests/rebuild_normals_command_native.rs',
    'crates/viboceros-command/tests/support/rebuild_normals.rs','crates/viboceros-command/tests/support/mesh_conversion.rs',
    'crates/viboceros-command/tests/support/mesh_command.rs','crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs','crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs','crates/viboceros-document/src/selection.rs',
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/normals.rs',
    'crates/viboceros-geometry/src/mesh/compaction.rs','crates/viboceros-geometry/src/mesh/vertex_channels.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h',
]+['tools/rhino_oracle/'+p for p in [
    'rebuild_normals_cases.py','rebuild_normals_probe.py','generate_rebuild_normals.py','report_rebuild_normals.py','test_rebuild_normals.py',
    'mesh_channel_capture.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py',
]])


def native():
    version=set();groups={'sdk':{},'command':{}}
    for family,count in FAMILIES.items():
        query=request(commands=family.endswith('command'),holdouts=family.endswith('holdouts'))
        data=json.loads((HERE/'observations'/(family+'.json')).read_text());version.add(data['engine_version'])
        if json.loads((HERE/'fixtures'/(family+'.json')).read_text())!=query or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in query['operations']]:raise ValueError('missing or changed normal rebuilding references')
        for row,op in zip(data['results'],query['operations']):
            command=op['op'].endswith('command');case=(COMMANDS if op['case']in COMMANDS else EXTRA_COMMANDS)if command else(SDK if op['case']in SDK else EXTRA_SDK)
            v=row.get('value')
            if v is None or v['input']!=case[row['id']]:raise ValueError('changed normal rebuilding input recipe')
            if command:
                accepted=not v['input'].get('cancel',False);events=[e for e in v['events']if e['name']=='RebuildMeshNormals']
                if v['success']is not accepted or len(events)!=1 or events[0]['result']!=('Success'if accepted else'Cancel'):raise ValueError('normal rebuilding command outcome differs')
            elif len(v['after']['normals'])!=len(v['after']['vertices'])or len(v['after']['face_normals'])!=len(v['after']['faces'])or v['after']['valid']is not True:raise ValueError('incomplete rebuilt normals')
            groups['command'if command else'sdk'][row['id']]=v
    if len(version)!=1:raise ValueError('mixed normal rebuilding versions')
    return version.pop(),groups


def summarize(text):
    version,groups=native();stages={};maximum=face_maximum=0.
    for stage in ['SDK','COMMAND','GETTER']:
        expected=groups['sdk'if stage=='SDK'else'command']
        if stage=='COMMAND':expected={k:v for k,v in expected.items()if v['success']}
        rows=[json.loads(s)for s in re.findall('^REBUILD_NORMALS_'+stage+r' (.+)$',text,re.M)]
        if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate normal rebuilding '+stage)
        for row in rows:
            v=expected[row['id']]
            flags=['qualified','raw_geometry','channels','normals','ngons','idempotent']if stage=='SDK'else['qualified','geometry','channels','normals','ngons','identity','metadata','selection','object_order','history']
            if any(row.get(k)is not True for k in flags):raise ValueError('unqualified normal rebuilding evidence')
            if stage=='SDK':
                for key,value in [('vertices',len(v['after']['vertices'])),('faces',len(v['after']['faces'])),('collapsed_face_queries',sum(all(c==0. for c in n)for n in v['after']['face_normals']))]:
                    if type(row.get(key))is not int or row[key]!=value:raise ValueError('incorrect normal rebuilding counts')
                delta=row.get('maximum_face_error')
                if type(delta)not in(int,float)or not math.isfinite(delta)or not 0.<=delta<=1e-6:raise ValueError('invalid face normal error')
                face_maximum=max(face_maximum,delta)
            elif row.get('accepted')is not v['success']:raise ValueError('incorrect normal rebuilding acceptance')
            maximum=max(maximum,error(row.get('maximum_error')))
        stages[stage.lower()]=sorted(rows,key=lambda r:list(expected).index(r['id']))
    sources=list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        sources.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_version=version,model_epsilon=1e-6,normal_component_epsilon=1e-6,sdk_cases=19,native_command_workflows=22,registered_command_cases=len(stages['command']),getter_cases=len(stages['getter']),maximum_error=maximum,maximum_derived_face_normal_error=face_maximum,full_native_parity=False,performance_qualified=False,**stages,
        limits=[
            'Nineteen independent public Mesh.RebuildNormals inputs compare exact raw vertices/faces, stored float normals, UV/colors and n-gons. Missing, zero and nonunit channels, unequal-area folds, nonmanifold edges, raw seams, opposed winding, small geometry, unused vertices and nondefault n-gon order are covered. Reapplying the operation is idempotent.',
            'Normal rebuilding also culls unused vertices in source order, including gaps before and between referenced vertices. Existing compaction remaps every channel and n-gon vertex list without merging coincident referenced vertices. Public OpenNURBS recomputes face normals and sums float face contributions into raw vertex normals; cancelling sums use world Z.',
            'Twenty-two native command workflows use exact saved source archives. Twenty accepted registered commands and twenty-two application getters compare raw geometry/channels, n-gons, identity, attributes/groups, object order, selection and Undo/Redo. Correct existing normals still record one replacement Undo and move meshes to the table end. Command-first picks clear; preselection survives acceptance and Undo, with processed selection released by Redo. Mixed SubDs are skipped.',
            'Native document admission populated normal channels despite clearing them before AddMesh; only SDK cases qualify initially absent stored normals. Two SDK cases repair invalid zero/nonunit normal channels. A collinear source face remains a stored record and contributes zero: native zero face normals correspond to an explicit declined unit-direction query locally. Noncollapsed derived face directions are compared within the component epsilon; cached face-normal storage is not modeled.',
            'Physical native postselection, restricted objects, grips, invalid index repair, general mesh repair, numeric extremes, other units and comparative performance remain unqualified. The command changes normals and removes unused vertices; it does not unify face orientation. Shared native resource limits apply after compaction.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(sources)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',type=Path,nargs='+');args=parser.parse_args()
    data=summarize('\n'.join(p.read_text()for p in args.logs));(ROOT/'docs/rebuild-normals-qualification.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
