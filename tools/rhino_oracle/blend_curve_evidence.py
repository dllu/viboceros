"""Audit adjustable curve-blend geometry, command results and retained limits."""
import gzip
import hashlib
import json
from . import blend_curve_command as recipes
from .arc_blend_radius_evidence import finite,read,HERE
from .arc_blend_subcurve_evidence import collect as prior_evidence
from .check_blend_curve import ROOT
BASELINE='a4be1aa9f3a80afa003cf93efac3e2e6ceff0e5c'
FAMILIES=dict(blend_curve_pilot=list(recipes.PILOT),blend_curve_discovery=list(recipes.DISCOVERY),blend_curve_holdouts=list(recipes.HOLDOUTS),blend_curve_adjustments=list(recipes.ADJUSTMENTS))
DRIVER={'g2_handle_start','g1_handle_start','flip_by_number'}
def diagnostic(name):return name.startswith(('g3_','g4_'))or name in DRIVER

def captures(proof):
    receipts=json.loads((proof/'captures.json').read_bytes());paths={p for p in proof.iterdir()if p.is_file()};summary=[];rows={};sha=lambda b:hashlib.sha256(b).hexdigest()
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory changed')
    for receipt,(family,names)in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
        if data['engine']!='rhino'or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names)or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native coverage or fixed request changed')
        paths.update([fixture,path])
        for digest in data['provenance']['sources'].values():
            p=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('frozen producer changed')
            paths.add(p)
        log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('incomplete BlendCrv capture')
        summary.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:rows[family+'/'+row['id']]=row['value']
    return rows,summary,paths

def normalized(objects):return sorted([{k:v for k,v in o.items()if k!='selected'}for o in objects],key=lambda o:o['attributes']['name'])
def command_result(v):
    events=[e for e in v['events']if e['name']=='BlendCrv']
    if len(events)!=1:raise ValueError('missing native command result')
    return events[0]['result']
def output(v):return next((o for o in v['after']if not o['attributes']['name']or o['native_type']=='PolyCurve'),None)

def collect(proof):
    rows,summary,paths=captures(proof);*_,old=prior_evidence(ROOT/'docs/arc-blend-subcurves-proof');paths.update(old);paths.add(ROOT/'docs/arc-blend-subcurves-validation.json')
    matched={};diagnostics={};picks=stations=0
    for key,v in rows.items():
        name=key.split('/',1)[1];recipe=recipes.CASES[name];trace=v['input_trace'][0]
        if v['prescribed_case']!=recipe or not v['returned_idle']:raise ValueError('BlendCrv recipe changed')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native blend geometry')
        result=command_result(v)
        if name in DRIVER:
            errors=['owned BlendCrv input timeout']+([]if name=='flip_by_number'else['incomplete owned BlendCrv input'])
            if trace['errors']!=errors or result!='Cancel'or v['history_changed']:raise ValueError('driver diagnostic classification changed')
            if name=='flip_by_number'and 'Unknown command: 1.0'not in v['history']:raise ValueError('missing rejected flip-number diagnostic')
            diagnostics[key]='input driver timeout';continue
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete BlendCrv workflow')
        if name.startswith(('g3_','g4_')):
            o=output(v)
            if result!='Nothing'or o is None or o['curve']['degree']!=(7 if name.startswith('g3_')else 9):raise ValueError('higher continuity classification changed')
            diagnostics[key]='higher endpoint derivatives remain unsupported';continue
        accepted=not name.startswith('cancel_')
        if v['success']is not False or result!=('Nothing'if accepted else'Cancel'):raise ValueError('native result semantics changed')
        if accepted:
            o=output(v)
            if not v['history_changed']or o is None or len(o['samples'])!=33:raise ValueError('missing native blend output or samples')
            if normalized(v['after'])!=normalized(v['redo'])or normalized(v['before'])!=normalized(v['undo']):raise ValueError('blend history changed')
            for sample in o['samples']:
                if len(sample['point'])!=3 or len(sample['tangent'])!=3:raise ValueError('incomplete blend station')
            stations+=33
            joined=o['native_type']=='PolyCurve'
            if joined:
                parent=next(x for x in v['before']if x['attributes']['name']=='guide0')
                if o['attributes']!=parent['attributes']or any(x['attributes']['name']=='guide1'for x in v['after']):raise ValueError('joined blend ownership changed')
            elif o['attributes']['layer']!='Plane output'or o['attributes']['groups']:raise ValueError('unjoined blend output attributes changed')
        elif v['history_changed']or normalized(v['before'])!=normalized(v['after'])or not v['pending_redo']:raise ValueError('cancellation changed geometry or lost pending redo')
        for click in trace.get('clicks',[]):
            if not click['motion_acknowledged']or not click['click_acknowledged']or len(click['ray'])!=2 or not finite(click):raise ValueError('invalid blend physical input')
            picks+=1
        matched[key]=v
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=1 or attempts[0]['exit_status']!=1 or attempts[0]['family']!='blend_curve_pilot':raise ValueError('failed dispatch attempt changed')
    attempt=attempts[0];log=gzip.decompress((proof/attempt['log']).read_bytes()).decode()
    if 'unsupported oracle operation: blend_curve_command'not in log:raise ValueError('missing failed dispatch receipt')
    for digest in attempt['sources'].values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:raise ValueError('failed producer changed')
        paths.add(p)
    if (len(matched),len(diagnostics),picks,stations)!=(25,9,52,759):raise ValueError('blend qualification coverage changed')
    return matched,diagnostics,summary,picks,stations,paths
