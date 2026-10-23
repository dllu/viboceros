"""Qualify exact SubD edge cuts/welds and shared Mesh/SubD command admission."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_vertex_seams import SOURCES as VERTEX_SOURCES
from .report_subd_box import HERE, ROOT, error

FAMILIES={'component_seams_subd':56,'component_seams_subd_edges':34,
          'component_seams_subd_edge_holdouts':14,'component_seams_subd_edge_contacts':5,
          'component_seams_subd_edge_tolerances_v2':7}
DIAGNOSTIC='component_seams_subd_edge_tolerances'
SOURCES=with_shared_sources(VERTEX_SOURCES+[
    'crates/viboceros-command/tests/subd_edge_seams_native.rs',
    'crates/viboceros-command/tests/edge_seams_native.rs',
    'crates/viboceros-command/tests/edge_seams_staging.rs',
    'crates/viboceros-command/tests/support/edge_seams.rs',
    'src/app/crease/tests/edge_seams.rs',
    'tools/rhino_oracle/report_edge_seams.py',
    'tools/rhino_oracle/test_edge_seams.py',
])


def observations():
    scenes={};engines=set()
    for family,count in FAMILIES.items():
        query=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        capture=json.loads((HERE/'observations'/(family+'.json')).read_text())
        rows=capture['results'];engines.add(capture['engine_version'])
        if len(rows)!=count or [r['id']for r in rows]!=[r['id']for r in query['operations']]:
            raise ValueError('missing or reordered native edge captures')
        for row in rows:
            value=row['value'];command=value['input']['command']
            if command not in ['WeldEdge','UnweldEdge']:continue
            if row['id']in scenes:raise ValueError('duplicate edge scene')
            if 'Unknown command:'in value['history']:raise ValueError('unknown native edge command')
            if value['success']is True:
                events=[e for e in value['events']if e['name']==command]
                if len(events)!=1 or events[0]['result']!='Success' or events[0]['objects']!=value['after']:
                    raise ValueError('invalid native edge lifecycle')
            scenes[row['id']]=value
    if len(scenes)!=86 or len(engines)!=1:raise ValueError('unexpected native edge corpus')
    return scenes,engines.pop()


def source_hashes():
    paths=list(SOURCES)
    for family in [*FAMILIES,DIAGNOSTIC]:
        paths.extend('tools/rhino_oracle/'+folder+'/'+family+'.json'for folder in ['fixtures','observations'])
        paths.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)}


def summarize(text):
    scenes,engine=observations()
    geometry=[json.loads(s)for s in re.findall(r'^SUBD_EDGE_GEOMETRY (.+)$',text,re.M)]
    commands=[json.loads(s)for s in re.findall(r'^EDGE_SEAM_COMMAND (.+)$',text,re.M)]
    getters=[json.loads(s)for s in re.findall(r'^EDGE_SEAM_GETTER (.+)$',text,re.M)]
    sources={(key,o['source'])for key,v in scenes.items()for o in v['before']if 'subd'in o}
    successful={key for key,v in scenes.items()if v['success']is True}
    getter_keys={(phase,key)for phase in ['preselected','postselected']for key in successful}
    if any(type(r.get('source'))is not int or r['source']<0 for r in geometry):
        raise ValueError('invalid SubD source ordinal')
    if len(geometry)!=94 or {(r['id'],r['source'])for r in geometry}!=sources:
        raise ValueError('missing, duplicate or unrelated SubD edge geometry')
    if len(commands)!=66 or {r['id']for r in commands}!=successful:
        raise ValueError('missing, duplicate or unrelated edge command evidence')
    if len(getters)!=132 or {(r['phase'],r['id'])for r in getters}!=getter_keys:
        raise ValueError('missing, duplicate or unrelated edge getter evidence')
    maximum=0.
    for row in geometry:
        maximum=max(maximum,error(row.get('maximum_error')))
        if row.get('qualified')is not True or any(type(row.get(k))is not bool for k in ['raw_control_ordinals_match','edge_ordinals_match']):
            raise ValueError('unqualified topology or control ordinal comparison')
        if type(row.get('changed_edges'))is not int or row['changed_edges']<0:
            raise ValueError('invalid changed edge count')
        value=scenes[row['id']];before=next(o for o in value['before']if o['source']==row['source'])
        after=next(o for o in value['after']if o['source']==row['source'])
        if (row['changed_edges']>0)!=(before['crc']!=after['crc']):raise ValueError('wrong native topology edit')
    for row in commands+getters:
        maximum=max(maximum,error(row.get('maximum_error')))
        for key in ['identity','attributes','history']+(['selection']if 'phase'in row else[]):
            if row.get(key)is not True:raise ValueError('unqualified edge '+key)
        if 'phase'not in row:
            value=scenes[row['id']];before={o['source']:o['crc']for o in value['before']if o['source']is not None}
            changed=sum(o['crc']!=before[o['source']]for o in value['after']if o['source']is not None)
            if type(row.get('processed_objects'))is not int or row['processed_objects']!=changed:
                raise ValueError('wrong processed edge owner count')
            if bool(changed)!=any(o['name']=='Undo witness'for o in value['undo']):raise ValueError('wrong native Undo witness')
    order={key:i for i,key in enumerate(scenes)}
    geometry.sort(key=lambda r:(order[r['id']],r['source']))
    commands.sort(key=lambda r:order[r['id']])
    getters.sort(key=lambda r:(r['phase'],order[r['id']]))
    return dict(native_engine_version=engine,model_epsilon=1e-6,maximum_error=maximum,
                geometry_source_cases=len(geometry),geometry_scenes=len(scenes),staged_command_cases=len(commands),getter_cases=len(getters),
                raw_control_ordinal_matches=sum(r['raw_control_ordinals_match']for r in geometry),
                edge_ordinal_matches=sum(r['edge_ordinals_match']for r in geometry),
                full_native_parity=False,performance_qualified=False,geometry=geometry,cases=commands,getters=getters,
                excluded_diagnostics={DIAGNOSTIC:'Seven immutable captures had no selected edges due to a producer closure bug. The corrected v2 family is qualified.'},
                limits=[
                    'Geometry comparisons permit a bijection of coincident control vertices distinguished by incident original faces. Original face IDs survive; sampled limit stations and density-two facets agree within epsilon. New control and edge numbering differs in some cuts, and subsequent component-index scripts may differ.',
                    'WeldEdge joins exact coincident boundary pairs within one SubD, retaining the selected side. One side suffices; gaps as small as 1e-10 and contacts between distinct SubDs make no edit. Vertex-only contacts do not weld.',
                    'UnweldEdge cuts smooth or creased manifold edges when radial face fans separate. An isolated cut on a closed fan is a no-op. ModifyNormals affects mesh channels but not SubD topology.',
                    'Sixty-six successful native command recipes qualify metadata, object order and atomic Undo/Redo, including mixed Mesh/SubD batches. Weld processes changed SubDs before meshes; Unweld processes changed meshes before SubDs.',
                    'The 132 frontend replays check preselection, postselection and retained SubD edge indices against native command outputs. They do not certify native physical mouse gestures or failed-command component selection lifecycles.',
                    'Wire/nonmanifold edges, ambiguous parallel source edges, more than two coincident boundary sides, arbitrary topology, continuous surface error and comparative performance remain unqualified.',
                ],sha256=source_hashes())


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path)
    args=parser.parse_args();report=summarize('\n'.join(p.read_text()for p in args.logs))
    (ROOT/'docs/edge-seams-qualification.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
