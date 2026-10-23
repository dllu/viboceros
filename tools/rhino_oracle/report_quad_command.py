"""Qualify native mesh command defaults, staged input and document lifecycle."""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from .quad_command_cases import request, CASES
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error
FAMILIES={'quad_command_discovery':3,'quad_command_main':17}
SOURCES=with_shared_sources([
    'Cargo.lock','crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/quadrangulate_mesh.rs',
    'crates/viboceros-command/src/remembered.rs','crates/viboceros-command/tests/quad_command_native.rs',
    'crates/viboceros-command/tests/support/quad_command.rs','crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs','crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs',
    'crates/viboceros-geometry/src/mesh.rs','src/app.rs',
]+['tools/rhino_oracle/'+p for p in [
    'quad_command_cases.py','quad_command_probe.py','generate_quad_command.py',
    'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
    'client.py','rhino_worker.py','structural_sdk.py','report_quad_command.py','test_quad_command.py',
]])


def native():
    result={}
    for family,count in FAMILIES.items():
        query=HERE/'fixtures'/(family+'.json')
        data=json.loads((HERE/'observations'/(family+'.json')).read_text())
        wanted=request(main=family.endswith('main'))
        if json.loads(query.read_text())!=wanted or len(data['results'])!=count:
            raise ValueError('native quad command request changed')
        if [r['id']for r in data['results']]!=[r['id']for r in wanted['operations']]:
            raise ValueError('missing or reordered native mesh workflow')
        preferences=dict(planarity=5.,rectangularity=2.)
        for row in data['results']:
            value=row.get('value')
            if value is None or value['input']!=CASES[row['id']]:raise ValueError('invalid native mesh command')
            completed=[e for e in value['events']if e['name']=='QuadrangulateMesh']
            accepted=not value['input'].get('cancel',False)
            if len(completed)!=1 or value['success']is not accepted or (completed[0]['result']=='Success')is not accepted:
                raise ValueError('unexpected native command outcome')
            prompts=re.findall(r'Planarity=([\d.eE+-]+)\s+Rectangularity=([\d.eE+-]+)',value['history'])
            if prompts:
                if tuple(map(float,prompts[0]))!=tuple(preferences.values()):raise ValueError('native remembered options differ')
                if accepted:preferences=dict(zip(preferences,map(float,prompts[-1])))
            if accepted and not prompts:raise ValueError('missing native options prompt')
            result[row['id']]=(value,dict(preferences))
    return result


def summarize(text):
    reference=native();stages={};maximum=0.
    for stage in ['COMMAND','GETTER']:
        expected={k:v for k,v in reference.items()if stage=='GETTER'or v[0]['success']}
        rows=[json.loads(s)for s in re.findall(r'^QUAD_'+stage+r' (.+)$',text,re.M)]
        if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate mesh command replay')
        for row in rows:
            value,preferences=expected[row['id']]
            for key in ['qualified','geometry','channels','identity','metadata','object_order','selection','history']:
                if row.get(key)is not True:raise ValueError('unqualified mesh workflow '+key)
            actual=row.get('preferences')
            if not isinstance(actual,dict)or set(actual)!=set(preferences)or any(type(v)not in(int,float)or not math.isfinite(v)for v in actual.values()):
                raise ValueError('invalid remembered mesh options')
            if row.get('accepted')is not value['success']or actual!=preferences:
                raise ValueError('native acceptance or remembered options differ')
            maximum=max(maximum,error(row.get('maximum_error')))
        stages[stage.lower()]=sorted(rows,key=lambda r:list(expected).index(r['id']))
    sources=list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        sources.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_versions=sorted({json.loads((HERE/'observations'/(f+'.json')).read_text())['engine_version']for f in FAMILIES}),
        model_epsilon=1e-6,native_workflows=20,command_cases=len(stages['command']),getter_cases=len(stages['getter']),
        maximum_error=maximum,fresh_defaults=dict(planarity=5.,rectangularity=2.),
        full_native_parity=False,performance_qualified=False,**stages,
        limits=[
            'Twenty native scripted command workflows use saved source 3DM files. Eighteen accepted commands and twenty application getter replays compare raw mesh records, stored normals/UV/colors, n-gons, identity, attributes, groups, selection, object table order and document Undo/Redo.',
            'Fresh private settings schemes start at Planarity=5 and Rectangularity=2. Accepted options persist across documents and Undo/Redo within one registry; cancelled edits do not replace them. Mixed mesh/SubD selections skip the SubD. Success clears selection and moves processed meshes to the table end.',
            'Successful commands record Undo even when conversion leaves all triangles unchanged. Cancel during selection or options admits no geometry/history and clears selection. Local malformed numeric updates are atomic, preserve redo and saved defaults, and leave the current getter active; stale geometry/tolerance rejects acceptance.',
            'Native sources and input recipes are independent of output geometry. Native postselection uses scripted All selection, not physical mouse input. The application provides text options without a native dialog or live conversion preview. Spinner Increment, locked/hidden sources, grip states, multi-mesh processing order, invalid native option ranges and comparative performance remain unqualified.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(sources)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    data=summarize('\n'.join(p.read_text()for p in args.logs))
    (ROOT/'docs/quad-command-qualification.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
