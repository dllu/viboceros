"""Qualify separate public SDK and TriangulateMesh command behavior."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .triangulate_cases import SDK,COMMANDS,HOLDOUTS,request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE,ROOT,error

FAMILIES={'triangulate_sdk':('sdk',17),'triangulate_command':('command',14),'triangulate_holdouts':('command',5)}
SOURCES=with_shared_sources([
    'Cargo.lock','src/app.rs','src/app/object_selection.rs','src/app/object_selection/triangulate_tests.rs',
    'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/mesh_edit/triangulate.rs',
    'crates/viboceros-command/src/tests/mesh_edit/triangulate.rs',
    'crates/viboceros-command/tests/triangulate_native.rs','crates/viboceros-command/tests/triangulate_command_native.rs',
    'crates/viboceros-command/tests/support/triangulate.rs','crates/viboceros-command/tests/support/mesh_command.rs',
    'crates/viboceros-command/tests/support/mesh_conversion.rs','crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs','crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs','crates/viboceros-document/src/selection.rs',
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/triangulate.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h',
]+['tools/rhino_oracle/'+p for p in [
    'triangulate_cases.py','triangulate_probe.py','generate_triangulate.py','report_triangulate.py','test_triangulate.py',
    'mesh_channel_capture.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
    'client.py','rhino_worker.py','structural_sdk.py',
]])


def native():
    groups={'sdk':{},'command':{}};versions=set()
    for family,(kind,count)in FAMILIES.items():
        query=request(commands=family=='triangulate_command',holdouts=family=='triangulate_holdouts')
        data=json.loads((HERE/'observations'/(family+'.json')).read_text());versions.add(data['engine_version'])
        if json.loads((HERE/'fixtures'/(family+'.json')).read_text())!=query or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in query['operations']]:raise ValueError('missing or changed triangulation inputs')
        cases=SDK if kind=='sdk'else HOLDOUTS if family.endswith('holdouts')else COMMANDS
        for row in data['results']:
            v=row.get('value')
            if v is None or v['input']!=cases[row['id']]:raise ValueError('native triangulation recipe differs')
            if kind=='sdk':
                if v['success']is not True or v['before']['valid']is not True or v['after']['valid']is not True:raise ValueError('invalid native SDK triangulation')
            else:
                events=[e for e in v['events']if e['name']=='TriangulateMesh'];accepted=not v['input'].get('cancel',False)
                if len(events)!=1 or v['success']is not accepted or events[0]['result']!=('Success'if accepted else'Cancel'):raise ValueError('native triangulation admission differs')
            groups[kind][row['id']]=v
    if len(versions)!=1:raise ValueError('mixed triangulation engines')
    return versions.pop(),groups


def summarize(text):
    version,groups=native();stages={};maximum=0.
    for stage in ['SDK','COMMAND','GETTER']:
        expected=groups['sdk'if stage=='SDK'else'command']
        if stage=='COMMAND':expected={k:v for k,v in expected.items()if v['success']}
        rows=[json.loads(s)for s in re.findall('^TRIANGULATE_'+stage+r' (.+)$',text,re.M)]
        if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate triangulation '+stage)
        for row in rows:
            v=expected[row['id']]
            flags=['qualified','raw_faces','channels','ngons','tolerance_independent']if stage=='SDK'else['qualified','geometry','channels','ngons','identity','metadata','selection','object_order','history']
            if any(row.get(k)is not True for k in flags):raise ValueError('unqualified triangulation evidence')
            if stage=='SDK':
                for key,value in [('converted',v['converted']),('vertices',len(v['after']['vertices'])),('faces',len(v['after']['faces']))]:
                    if type(row.get(key))is not int or row[key]!=value:raise ValueError('incorrect SDK triangulation count')
            elif row.get('accepted')is not v['success']:raise ValueError('incorrect triangulation acceptance')
            maximum=max(maximum,error(row.get('maximum_error')))
        stages[stage.lower()]=sorted(rows,key=lambda r:list(expected).index(r['id']))
    sources=list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        sources.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_version=version,model_epsilon=1e-6,sdk_cases=17,native_command_workflows=19,registered_command_cases=len(stages['command']),getter_cases=len(stages['getter']),maximum_error=maximum,full_native_parity=False,performance_qualified=False,**stages,
        limits=[
            'Independent public ConvertQuadsToTriangles calls cover seventeen raw input meshes: shortest diagonals, exact spatial ties, reversed/cyclic records, concave/nonplanar faces, translated and tiny geometry, stored colors/UV/normals, unused vertices, mixed face slots and differently ordered n-gon tables. Two local document tolerances produce the same SDK geometry.',
            'Nineteen native TriangulateMesh command workflows retain exact saved source 3DM files. Seventeen accepted registered commands and nineteen application getters compare raw geometry/channels, n-gons, identity, attributes/groups, selection, object order and Undo/Redo. Selection-first and command-first workflows are distinct; cancellation creates no model history.',
            'SDK conversion retains n-gons and appends new face indices after each existing face list. The command removes all n-gon overlays, including triangle-only and otherwise unaffected regions. Accepted commands record replacements and move processed meshes to the table end even for triangle-only input. Preselected meshes stay selected after acceptance; command-first picks are released. Redo releases processed selections; unrelated preselected objects are skipped.',
            'Only public APIs and scripted command selection are used. Physical postselection, restricted objects, grips, malformed/degenerate source repair, coordinates near numeric limits, oversized input, other units and comparative speed remain unqualified. Source input remains bounded by the shared native mesh resource limits.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(sources)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',type=Path,nargs='+');args=parser.parse_args()
    data=summarize('\n'.join(p.read_text()for p in args.logs));(ROOT/'docs/triangulate-qualification.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
