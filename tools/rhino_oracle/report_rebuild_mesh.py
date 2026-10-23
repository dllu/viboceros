"""Qualify independent mesh reconstruction and staged preservation settings."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .rebuild_mesh_cases import SDK,DISCOVERY,COMMANDS,HOLDOUTS,request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE,ROOT,error
EXCLUDED='rebuild_mesh_discovery_preselected_flags'
FAMILIES={'rebuild_mesh_sdk':15,'rebuild_mesh_discovery':6,'rebuild_mesh_command':21,'rebuild_mesh_holdouts':6}
SOURCES=with_shared_sources([
    'Cargo.lock','src/app.rs','src/app/object_selection.rs','src/app/object_selection/rebuild_mesh_tests.rs',
    'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/mesh_edit/mod.rs',
    'crates/viboceros-command/src/mesh_edit/rebuild_mesh.rs','crates/viboceros-command/src/mesh_edit/rebuild_mesh/tests.rs',
    'crates/viboceros-command/src/mesh_edit/replacement.rs','crates/viboceros-command/src/remembered.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/src/tests/document/registry.rs',
    'crates/viboceros-command/tests/rebuild_mesh_native.rs','crates/viboceros-command/tests/rebuild_mesh_command_native.rs',
    'crates/viboceros-command/tests/support/rebuild_mesh.rs','crates/viboceros-command/tests/support/mesh_command.rs',
    'crates/viboceros-command/tests/support/mesh_conversion.rs','crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs','crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs','crates/viboceros-document/src/selection.rs',
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/rebuild_mesh.rs',
    'crates/viboceros-geometry/src/mesh/normals.rs','crates/viboceros-geometry/src/mesh/compaction.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h',
]+['tools/rhino_oracle/'+p for p in [
    'rebuild_mesh_cases.py','rebuild_mesh_probe.py','generate_rebuild_mesh.py','report_rebuild_mesh.py','test_rebuild_mesh.py',
    'mesh_channel_capture.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py',
]])


def staged(value,before):
    pairs=re.findall(r'PreserveTextureCoordinates=(Yes|No)\s+PreserveVertexColors=(Yes|No)',value['history'])
    if pairs and tuple(v=='Yes'for v in pairs[0])!=tuple(before.values()):raise ValueError('native mesh rebuild preferences changed')
    return dict(zip(before,(v=='Yes'for v in pairs[-1])))if pairs else dict(before)


def native():
    versions=set();groups={'sdk':{},'command':{}}
    for family,count in FAMILIES.items():
        flags={family.removeprefix('rebuild_mesh_'):True}if family!='rebuild_mesh_sdk'else{}
        if 'command'in flags:flags={'commands':True}
        query=request(**flags);data=json.loads((HERE/'observations'/(family+'.json')).read_text());versions.add(data['engine_version'])
        if json.loads((HERE/'fixtures'/(family+'.json')).read_text())!=query or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in query['operations']]:raise ValueError('missing or changed mesh rebuild references')
        cases=SDK if family.endswith('sdk')else DISCOVERY if family.endswith('discovery')else COMMANDS if family.endswith('command')else HOLDOUTS
        before=dict(preserve_texture_coordinates=False,preserve_vertex_colors=False)
        for row in data['results']:
            v=row.get('value')
            if v is None or v['input']!=cases[row['id']]:raise ValueError('mesh rebuild input recipe differs')
            if family.endswith('sdk'):
                if v['success']is not True or v['before']!=v['source_after']or v['after']['valid']is not True or v['after']['ngons']:raise ValueError('invalid static mesh rebuild reference')
                groups['sdk'][row['id']]=v;continue
            if row['id']==EXCLUDED:
                if 'Unknown command: _PreserveTextureCoordinates=_Yes'not in v['history']:raise ValueError('late preselection option was relabelled')
                continue
            accepted=not v['input'].get('cancel',False);events=[e for e in v['events']if e['name']=='RebuildMesh']
            if v['success']is not accepted or len(events)!=1 or events[0]['result']!=('Success'if accepted else'Cancel')or'Unknown command'in v['history']:raise ValueError('native rebuild command admission differs')
            pending=staged(v,before)
            if accepted:before=pending
            if accepted:
                for source in [o for o in v['before']if'mesh'in o]:
                    after=next(o['mesh']for o in v['after']if o['source']==source['source'])
                    if after['ngons']:raise ValueError('native command retained n-gons')
                    for key,flag in [('uv','preserve_texture_coordinates'),('colors','preserve_vertex_colors')]:
                        if source['mesh'][key]and len(after[key]or[])!=(len(after['vertices'])if before[flag]else 0):raise ValueError('native preserved channel differs from preferences')
            groups['command'][row['id']]=(v,dict(before))
    if len(versions)!=1:raise ValueError('mixed mesh rebuild engines')
    return versions.pop(),groups


def summarize(text):
    version,groups=native();stages={};maximum=0.
    for stage in ['SDK','COMMAND','GETTER']:
        expected=groups['sdk'if stage=='SDK'else'command']
        if stage=='COMMAND':expected={k:v for k,v in expected.items()if v[0]['success']}
        rows=[json.loads(s)for s in re.findall('^REBUILD_MESH_'+stage+r' (.+)$',text,re.M)]
        if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate mesh rebuild '+stage)
        for row in rows:
            v=expected[row['id']]if stage=='SDK'else expected[row['id']][0]
            flags=['qualified','geometry','channels','normals','ngons','source_unchanged','idempotent']if stage=='SDK'else['qualified','geometry','channels','normals','ngons','identity','metadata','selection','object_order','history','staged']
            if any(row.get(k)is not True for k in flags):raise ValueError('unqualified mesh rebuild evidence')
            if stage=='SDK':
                for key,wanted in [('vertices',len(v['after']['vertices'])),('faces',len(v['after']['faces']))]:
                    if type(row.get(key))is not int or row[key]!=wanted:raise ValueError('incorrect mesh rebuild counts')
                options=v['input']['options']or[False,False]
                if row.get('preserve_uv')is not options[0]or row.get('preserve_colors')is not options[1]:raise ValueError('incorrect mesh rebuild preservation flags')
            elif row.get('accepted')is not v['success']or row.get('preferences')!=expected[row['id']][1]or any(type(x)is not bool for x in row['preferences'].values()):raise ValueError('incorrect mesh rebuild preferences')
            maximum=max(maximum,error(row.get('maximum_error')))
        stages[stage.lower()]=sorted(rows,key=lambda r:list(expected).index(r['id']))
    sources=list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        sources.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_version=version,model_epsilon=1e-6,sdk_cases=15,native_command_workflows=32,registered_command_cases=len(stages['command']),getter_cases=len(stages['getter']),excluded_discovery_workflows=1,maximum_error=maximum,full_native_parity=False,performance_qualified=False,**stages,
        limits=[
            'Fifteen independent public static Mesh.RebuildMesh inputs compare exact raw geometry, stored float normals, UV/color channels and n-gon removal. Both overloads and all four preservation combinations are covered, including interspersed unused vertices, coincident raw seams, reversed/opposed and collinear faces, zero source normals, tiny geometry and translated double-precision points. Native and local sources remain unchanged; rebuilt results are idempotent.',
            'Thirty-two supported native command workflows use saved source archives. Twenty-eight accepted registered commands and thirty-two application getters compare raw mesh state, identity, attributes/groups, selection, object order and Undo/Redo. Defaults start at No/No. Accepted options persist across documents and model history; cancelled updates do not replace them. Bare names toggle the active preservation value.',
            'Options are staged during object selection. Preselected native invocation executes immediately with accepted defaults. One late-option discovery becomes an unknown command after native execution and qualifies no geometry. Local complete script invocations additionally accept preservation arguments with preselected objects; that extension does not qualify the rejected native token sequence.',
            'Rebuilding compacts unused vertices in source order and remaps retained channels, removes every n-gon overlay, and uses public OpenNURBS normal reconstruction. Referenced coincident vertices and face winding remain distinct. Processed replacements move to the table end and record Undo even for unchanged geometry; preselection survives acceptance/Undo while command-first picks and Redo selection are released. Non-mesh selections are skipped.',
            'Surface curvatures, surface parameter channels and cached face-normal storage are not represented by the mesh model. Physical native postselection, restricted objects, grips, invalid-index repair, broader mesh repair, numeric extremes, other units and comparative performance remain unqualified. Native mesh resource limits apply after compaction.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(sources)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',type=Path,nargs='+');args=parser.parse_args()
    data=summarize('\n'.join(p.read_text()for p in args.logs));(ROOT/'docs/rebuild-mesh-qualification.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
