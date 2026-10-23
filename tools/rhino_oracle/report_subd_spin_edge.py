"""Qualify saved edge spins and physical native repeat/local-Undo workflows."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_edge_seams import SOURCES as EDGE_SOURCES
from .report_subd_box import HERE,ROOT,error
FAMILIES={'subd_spin_edge_discovery':4,'subd_spin_edge':19,'subd_spin_edge_live':12,
          'subd_spin_edge_live_v2':12,'subd_spin_edge_live_v3':15,'subd_spin_edge_holdouts':4,'subd_spin_edge_live_v5':4}
SOURCES=with_shared_sources(EDGE_SOURCES+[
    'crates/viboceros-command/tests/subd_spin_edge_native.rs',
    'crates/viboceros-command/tests/subd_spin_edge_command.rs',
    'crates/viboceros-command/tests/subd_spin_edge_staging.rs',
    'crates/viboceros-command/tests/support/subd_spin_edge.rs',
    'crates/viboceros-document/src/object_geometry.rs',
    'tools/rhino_oracle/subd_spin_edge_cases.py','tools/rhino_oracle/subd_spin_edge_probe.py',
    'tools/rhino_oracle/generate_subd_spin_edge.py','tools/rhino_oracle/catenary_point_picker.py',
    'tools/rhino_oracle/group_picking.py','tools/rhino_oracle/subd_edit_scene.py',
    'tools/rhino_oracle/fixtures/subd_spin_edge_live_v4.json',
    'tools/rhino_oracle/observations/subd_spin_edge_live_v4_timeout.json',
    'tools/rhino_oracle/observations/subd_spin_edge_live_v4_timeout.log',
    'tools/rhino_oracle/report_subd_spin_edge.py','tools/rhino_oracle/test_subd_spin_edge.py',
])


def observations():
    expected={};excluded={};engines=set();core={}
    for family,count in FAMILIES.items():
        query=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        capture=json.loads((HERE/'observations'/(family+'.json')).read_text());engines.add(capture['engine_version'])
        rows=capture['results']
        if len(rows)!=count or [r['id']for r in rows]!=[r['id']for r in query['operations']]:raise ValueError('missing or reordered edge spin observations')
        for row in rows:
            key=row['id'];v=row.get('value')
            if v is None:
                excluded[family+':'+key]='Producer error retained as a diagnostic.';continue
            if family=='subd_spin_edge' and v['success']is True:core[key]=v
            if family in ['subd_spin_edge_discovery','subd_spin_edge_live','subd_spin_edge_live_v2']:
                excluded[family+':'+key]='Discovery or failed input-control capture; retained unchanged.';continue
            reason=None
            if key=='spin_live_v5_undo_both':
                before={o['source']:o for o in v['before']if 'subd'in o};after={o['source']:o for o in v['after']if 'subd'in o}
                if after[1]['subd']['points']!=before[0]['subd']['points']or before[1]['subd']['points']==before[0]['subd']['points']:raise ValueError('native Undo transplant diagnostic changed')
                excluded[family+':'+key]='Native local Undo transplanted object 0 geometry onto object 1. Viboceros restores each actual source object.';continue
            if any(o.get('subd',{}).get('valid')is False for o in v['after']):reason='Native output is invalid; valid tag repair remains an explicit native difference.'
            elif 'Unknown command'in v['history']:reason='Native input included an unavailable option or a token outside the command.'
            elif any(t.get('errors')or t.get('completed')!=t.get('expected')for t in v.get('input_trace',[])):reason='Incomplete native physical input.'
            elif not v['input'].get('live')and (v['input'].get('command_arguments')is not None or v['input'].get('cancel')):reason='Trailing preselection tokens are outside the interactive command.'
            if reason:excluded[family+':'+key]=reason;continue
            events=[e for e in v['events']if e['name']=='SubDSpinEdge']
            if len(events)!=1 or events[0]['objects']!=v['after'] or events[0]['result']not in ['Success','Cancel','Failure']:raise ValueError('invalid native edge spin lifecycle: '+key)
            if key in expected:raise ValueError('duplicate qualified spin id')
            expected[key]=v
    if len(engines)!=1 or len(core)!=17:raise ValueError('unexpected edge spin corpus')
    return expected,core,excluded,engines.pop()


def source_hashes():
    paths=list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+folder+'/'+family+'.json'for folder in ['fixtures','observations'])
        paths.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)}


def summarize(text):
    expected,core,excluded,engine=observations()
    commands=[json.loads(s)for s in re.findall(r'^SUBD_SPIN_COMMAND (.+)$',text,re.M)]
    getters=[json.loads(s)for s in re.findall(r'^SUBD_SPIN_GETTER (.+)$',text,re.M)]
    undo_gap=[json.loads(s)for s in re.findall(r'^SUBD_SPIN_UNDO_GAP (.+)$',text,re.M)]
    if len(undo_gap)!=1 or undo_gap[0].get('id')!='spin_live_v5_undo_both' or undo_gap[0].get('native_match')is not False or any(undo_gap[0].get(k)is not True for k in ['source_locations_restored','history']):raise ValueError('missing native cross-object Undo gap regression')
    geometry=[json.loads(s)for s in re.findall(r'^SUBD_SPIN_DISCOVERY (.+)$',text,re.M)]
    for name,rows,wanted in [('command',commands,expected),('getter',getters,expected),('geometry',geometry,core)]:
        if len(rows)!=len(wanted)or {r['id']for r in rows}!=set(wanted):raise ValueError('missing, duplicate or unrelated spin '+name+' evidence')
    maximum=0.
    for row in geometry:
        valid=all(o['subd']['valid']for o in core[row['id']]['after']if 'subd'in o)
        if row.get('native_valid')is not valid or row.get('repaired')is not(not valid)or row.get('valid_output')is not True:raise ValueError('incorrect native validity or repair claim')
        if valid:maximum=max(maximum,error(row.get('maximum_error')))
        elif row.get('maximum_error')is not None:raise ValueError('invalid native output cannot qualify a numeric surface comparison')
    for row in commands+getters:
        maximum=max(maximum,error(row.get('maximum_error')))
        for key in ['identity','metadata','history']+(['selection']if row in getters else['preview']):
            if row.get(key)is not True:raise ValueError('unqualified spin '+key)
        if row in commands:
            v=expected[row['id']];history=any(o['name']=='Undo witness'for o in v['undo'])
            owners=len({a.get('source',0)for a in v['input'].get('actions',[])if isinstance(a,dict)})if v['input'].get('live')else 1
            wanted=owners if history else 0
            if type(row.get('processed_objects'))is not int or row['processed_objects']!=wanted:raise ValueError('incorrect spin history admission')
    order={key:i for i,key in enumerate(expected)}
    commands.sort(key=lambda r:order[r['id']]);getters.sort(key=lambda r:order[r['id']]);geometry.sort(key=lambda r:list(core).index(r['id']))
    return dict(native_engine_version=engine,model_epsilon=1e-6,maximum_error=maximum,
                staged_command_cases=len(commands),getter_cases=len(getters),geometry_cases=len(geometry),
                captured_workflows=sum(FAMILIES.values()),timeout_workflows=6,cross_object_undo=undo_gap,native_invalid_output_cases=sum(not r['native_valid']for r in geometry),
                cases=commands,getters=getters,geometry=geometry,excluded=excluded,
                full_native_parity=False,performance_qualified=False,
                limits=[
                    'Spins reuse public OpenNURBS topology edits on saved source geometry. Valid native control topology, tags, sampled limit stations and density-two facets agree within epsilon. Original control, edge and face identities remain stable.',
                    'Native physical command-first captures exercise clockwise and counterclockwise spins, Direction toggles, repeated spins, local Undo, Enter, Escape and multi-object order. Escape retains accepted spins. Undoing all spins still records one document-history entry.',
                    'One preselected edge spins immediately. Multiple preselected edges are ignored and command-first picking begins. Boundary edges make no geometry/history edit.',
                    'Four native hard-crease/corner outputs have invalid vertex tags. Viboceros repairs those tags and emits a valid archive; these cases do not claim native tag or surface parity.',
                    'Failed input drivers and out-of-command option tokens remain immutable diagnostics. Command result Cancel with local Undo is retained rather than being relabeled Success.',
                    'A captured cross-object local Undo transplanted the first object geometry onto the second object. Viboceros restores the actual source snapshot; native parity is not claimed for this erroneous result.',
                    'The owned native grip-enabled probe stalled without a response and was terminated by its runner. Grip-state parity remains unqualified; the request, timeout and frozen producer sources are retained. Arbitrary topology, continuous surface error and comparative performance remain unqualified.',
                ],sha256=source_hashes())


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    (ROOT/'docs/subd-spin-edge-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
