"""Audit temporary parents, fixed getter recipes and complete native captures."""
import gzip
import hashlib
import json
from . import arc_blend_command as recipes
from .arc_blend_radius_evidence import finite,read,HERE
from .arc_blend_source_evidence import collect as prior_evidence
from .check_arc_blend_subcurves import ROOT
BASELINE='7f733125347d48aaa31586a449c49c86cf7cf1ae'
FAMILIES=dict(arc_blend_subcurve_discovery=list(recipes.SUBCURVE_DISCOVERY),arc_blend_subcurve_holdouts=list(recipes.SUBCURVE_HOLDOUTS),arc_blend_subcurve_pointers=list(recipes.SUBCURVE_POINTERS),arc_blend_subcurve_closed=list(recipes.SUBCURVE_CLOSED))

def captures(proof):
    receipts=json.loads((proof/'captures.json').read_bytes());evidence={p for p in proof.iterdir()if p.is_file()};summary=[];rows={};sha=lambda b:hashlib.sha256(b).hexdigest()
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory changed')
    for receipt,(family,names)in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
        if data['engine']!='rhino'or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names)or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native coverage or fixed request changed')
        evidence.update([fixture,path])
        if 'arc_blend_subcurve_cases.py'not in data['provenance']['sources']:raise ValueError('missing frozen subcurve producer')
        for digest in data['provenance']['sources'].values():
            source=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(source.read_bytes()))!=digest:raise ValueError('frozen producer changed')
            evidence.add(source)
        log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('incomplete subcurve capture')
        summary.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:rows[family+'/'+row['id']]=row['value']
    return rows,summary,evidence

def normalized(objects):
    return sorted([{k:v for k,v in o.items()if k!='selected'}for o in objects],key=lambda o:o['attributes']['name'])

def collect(proof):
    rows,summary,evidence=captures(proof)
    *_,old=prior_evidence(ROOT/'docs/arc-blend-sources-proof');evidence.update(old);evidence.add(ROOT/'docs/arc-blend-sources-validation.json')
    picks=stations=0
    for key,v in rows.items():
        name=key.split('/',1)[1];recipe=recipes.CASES[name];trace=v['input_trace'][0];accepted=name!='subcurve_cancel'
        if v['prescribed_case']!=recipe:raise ValueError('subcurve recipe changed')
        if v['success']is not accepted or not v['returned_idle']or trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete subcurve workflow')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in (v[phase]or [])):raise ValueError('invalid native subcurve geometry')
        if accepted:
            if normalized(v['after'])!=normalized(v['redo'])or normalized(v['before'])!=normalized(v['undo']):raise ValueError('subcurve history changed')
            output=[o for o in v['after']if not o['attributes']['name']]
            if len(output)!=1:raise ValueError('missing subcurve output')
            arcs=output[0].get('arc_segments')
            if arcs:
                if len(arcs)not in(2,4)or any(len(o['samples'])!=33 for o in [output[0],*arcs]):raise ValueError('incomplete independent subcurve samples')
                stations+=33*(1+len(arcs))
        elif normalized(v['before'])!=normalized(v['after'])or not v['pending_redo']:raise ValueError('cancel changed geometry or lost pending redo')
        # Every temporary parent survives unless that same object is also used
        # as a whole guide and consumed by Join. This checks input ownership,
        # independently of our implementation's output comparison.
        temporary=set();whole=set();nested=False;options={}
        for action in recipe['actions']:
            if action=='SubCrv':nested=True
            elif isinstance(action,dict)and 'curve'in action:
                (temporary if nested else whole).add(action['curve']);nested=False
            elif isinstance(action,str)and '='in action:
                a,b=action.split('=',1);options[a]=b
        for i in temporary-whole:
            parent=next(o for o in v['before']if o['attributes']['name']=='guide'+str(i))
            after=next((o for o in v['after']if o['attributes']['name']=='guide'+str(i)),None)
            if after is None or normalized([parent])!=normalized([after]):raise ValueError('temporary parent changed')
        if accepted and options.get('Trim')=='Yes'and options.get('Join')=='Yes':
            if any(o['attributes']['name']=='guide'+str(i)for i in whole for o in v['after']):raise ValueError('whole joined source retained')
        for click in trace.get('clicks',[]):
            if not click['motion_acknowledged']or not click['click_acknowledged']or len(click['ray'])!=2 or not finite(click):raise ValueError('invalid subcurve physical input')
            picks+=1
    if len(rows)!=29:raise ValueError('subcurve coverage changed')
    return rows,summary,picks,stations,evidence
