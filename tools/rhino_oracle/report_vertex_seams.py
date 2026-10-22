"""Qualify staged vertex seam edits and frontend replay of fixed native scenes."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_crease import SOURCES as CREASE_SOURCES
from .report_subd_box import HERE, ROOT, error

FAMILIES={'component_seams_subd':56,'component_seams_subd_followup':16,'component_seams_subd_grips':9}
SOURCES=with_shared_sources(CREASE_SOURCES+[
    'crates/viboceros-command/src/control_points.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/src/vertex_seams.rs',
    'crates/viboceros-command/src/vertex_seams/arguments.rs',
    'crates/viboceros-command/tests/vertex_seams_native.rs',
    'crates/viboceros-command/tests/vertex_seams_staging.rs',
    'crates/viboceros-command/tests/subd_surface_grips_native.rs',
    'crates/viboceros-command/tests/support/vertex_seams.rs',
    'crates/viboceros-document/src/control_points/geometry.rs',
    'src/app/crease/tests/vertex_seams.rs',
    'tools/rhino_oracle/component_seams_subd_cases.py',
    'tools/rhino_oracle/component_seams_subd_probe.py',
    'tools/rhino_oracle/generate_component_seams_subd.py',
    'tools/rhino_oracle/report_vertex_seams.py',
    'tools/rhino_oracle/test_vertex_seams.py',
    'tools/rhino_oracle/fixtures/subd_surface_grip_gaps.json',
])


def observations():
    expected={};engines=set()
    for family,count in FAMILIES.items():
        request=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        capture=json.loads((HERE/'observations'/(family+'.json')).read_text())
        rows=capture['results'];engines.add(capture['engine_version'])
        if len(rows)!=count or [r['id']for r in rows]!=[o['id']for o in request['operations']]:
            raise ValueError('missing or reordered native vertex recipes')
        for row in rows:
            value=row['value']
            if value['input']['command']not in ['WeldVertices','UnweldVertex']:continue
            events=[e for e in value['events']if e['name']==value['input']['command']]
            if (value['success']is not True or len(events)!=1 or events[0]['result']!='Success'
                    or events[0]['objects']!=value['after'] or 'Unknown command:'in value['history']):
                raise ValueError('invalid native vertex lifecycle')
            if row['id']in expected:raise ValueError('duplicate native vertex recipe')
            expected[row['id']]=value
    if len(engines)!=1 or len(expected)!=46:raise ValueError('unexpected vertex observations')
    return expected,engines.pop()


def source_hashes():
    paths=list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+folder+'/'+family+'.json'for folder in ['fixtures','observations'])
        paths.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)}


def summarize(text):
    expected,engine=observations()
    commands=[json.loads(s)for s in re.findall(r'^VERTEX_SEAM_COMMAND (.+)$',text,re.M)]
    getters=[json.loads(s)for s in re.findall(r'^VERTEX_SEAM_GETTER (.+)$',text,re.M)]
    grips=[json.loads(s)for s in re.findall(r'^SUBD_SURFACE_GRIP (.+)$',text,re.M)]
    grip_expected=json.loads((HERE/'observations/component_seams_subd_grips.json').read_text())['results']
    gaps=json.loads((HERE/'fixtures/subd_surface_grip_gaps.json').read_text())
    if len(grips)!=9 or {r['id']for r in grips}!={r['id']for r in grip_expected}:
        raise ValueError('missing, duplicate or unrelated surface grip comparisons')
    grip_by_id={r['id']:r for r in grips};grips=[grip_by_id[r['id']]for r in grip_expected]
    for row in grips:
        match=row['id']not in gaps
        if row.get('native_match')is not match or any(row.get(k)is not True for k in ['untouched_controls','tags_and_topology']):
            raise ValueError('unqualified surface grip topology or native match')
        error(row.get('selected_station_error'))
        for k in ['maximum_error','native_selected_station_error']:
            value=row.get(k)
            if match:error(value)
            elif type(value)not in [int,float]or not 1e-6<value<1e100:raise ValueError('incorrect known surface grip gap')
    wanted={key for key,value in expected.items()if all(s['kind']=='subd'for s in value['input']['sources'])}
    if len(commands)!=46 or {r['id']for r in commands}!=set(expected):
        raise ValueError('missing, duplicate or unrelated staged vertex comparisons')
    keys={(phase,key)for phase in ['preselected','postselected']for key in wanted}
    if len(getters)!=80 or {(r['phase'],r['id'])for r in getters}!=keys:
        raise ValueError('missing, duplicate or unrelated vertex getter comparisons')
    maximum=0.
    for row in commands+getters:
        for name in ['identity','attributes','history']+(['selection']if 'phase'in row else[]):
            if row.get(name)is not True:raise ValueError('unqualified vertex '+name)
        maximum=max(maximum,error(row.get('maximum_error')))
        if 'phase'not in row:
            value=expected[row['id']]
            before={o['source']:o['crc']for o in value['before']if o['source']is not None}
            changed=sum(o['crc']!=before[o['source']]for o in value['after']if o['source']is not None)
            if type(row.get('processed_objects'))is not int or row['processed_objects']!=changed:
                raise ValueError('incorrect processed vertex owners')
            if bool(changed)!=any(o['name']=='Undo witness'for o in value['undo']):
                raise ValueError('native vertex Undo witness mismatch')
    commands.sort(key=lambda r:list(expected).index(r['id']))
    getters.sort(key=lambda r:(r['phase'],list(expected).index(r['id'])))
    return dict(native_engine_version=engine,model_epsilon=1e-6,maximum_error=maximum,
                staged_command_cases=len(commands),getter_cases=len(getters),
                captured_workflows=sum(FAMILIES.values()),edge_diagnostic_cases=26,
                surface_grip_cases=len(grips),surface_grip_native_matches=6,surface_grips=grips,surface_grip_gaps=gaps,
                full_native_parity=False,performance_qualified=False,cases=commands,getters=getters,
                limits=[
                    'Forty-six staged vertex command recipes qualify saved source geometry, control topology, tags, sampled limit stations and facets, identity, attributes, order, grips and Undo/Redo. Six include mixed Mesh/SubD inputs.',
                    'Eighty frontend getter replays cover the forty pure-SubD recipes before and after command start. They are regressions against native command outputs, not physical native mouse-gesture parity.',
                    'WeldVertices ignores SubD boundary vertices and already-smooth soft vertices, clears vertex/grip selection before history admission, and processes SubDs before meshes. UnweldVertex creates corners, preserves grips and parent selection, and processes changed objects in document order.',
                    'Mesh comparison permits raw-index bijections and cyclic face origins; mesh normal arrays and texture channels are outside this corpus. Existing mesh seam qualification records remain separate.',
                    'The twenty-six edge command captures are diagnostics. SubD WeldEdge automation did not edit pure SubDs, and SubD UnweldEdge can cut crease topology. They do not qualify either SubD edge command.',
                    'Nine surface-grip move captures check restricted control edits and exact selected-station targets. Six match native controls and sampled facets; two weighted-edge cases and an unstable all-selected cube solve retain explicit native gaps. Whole-object affine edits use the exact affine map.',
                    'Arbitrary topology, continuous limit-surface error, native gesture parity and comparative performance remain unqualified.',
                ],sha256=source_hashes())


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path)
    args=parser.parse_args();report=summarize('\n'.join(p.read_text()for p in args.logs))
    (ROOT/'docs/vertex-seams-qualification.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
