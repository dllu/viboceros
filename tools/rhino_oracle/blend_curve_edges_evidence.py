"""Audit surface-mode inputs separately from ordinary boundary-curve picks."""
import gzip
import hashlib
import json
import math
from . import blend_curve_command as recipes,blend_curve_symmetry_evidence as prior
from .blend_curve_high_evidence import captures as read_captures
from .blend_curve_evidence import command_result,normalized,output
from .arc_blend_radius_evidence import finite
from .check_blend_curve_edges import ROOT
BASELINE='1eaf845b485d916b1cd0f5bf220ce5d659fa5187'
FAMILIES=dict(blend_curve_edge_pilot=list(recipes.EDGE_PILOT),blend_curve_edge_selection=list(recipes.EDGE_SELECTION),blend_curve_edge_discovery=list(recipes.EDGE_DISCOVERY),blend_curve_edge_holdouts=list(recipes.EDGE_HOLDOUTS))
DIAGNOSTICS={**{n:'selector discovery includes rejected options'for n in recipes.EDGE_SELECTION},'surface_cancel':'extra Enter after cancellation','surface_curved_g3':'curved-face G3 remains guarded','surface_curved_g4':'curved-face G4 remains guarded'}
def captures(proof):return read_captures(proof,FAMILIES)
def inputs(value):
    result=[];surface=False
    for index,a in enumerate(value['prescribed_case']['actions']):
        if a=='Edges':surface=True
        elif a=='Curves':surface=False
        elif isinstance(a,dict)and 'curve'in a:result.append((index,'guide'+str(a['curve']),surface))
        elif a=='Point':result.append((index,None,False))
    return result
def nearest(click):
    context=click.get('edge_pick_context',{});candidates=context.get('candidates',[])
    if not finite(context)or not candidates or context.get('subobjects')is not True:raise ValueError('missing public edge input receipt')
    candidates=sorted(candidates,key=lambda c:c['distance'])
    if len(candidates)>1 and candidates[1]['distance']-candidates[0]['distance']<=1e-8:raise ValueError('ambiguous edge input receipt')
    c=candidates[0]
    if (not 0<=c['edge']<len(context['topology'])or len(c['point'])!=3 or len(c['tangent'])!=3
            or abs(math.dist(c['tangent'],[0.,0.,0.])-1.)>1e-10 or len(c['faces'])!=1):raise ValueError('invalid edge input receipt')
    f=c['faces'][0]
    if not f['found']or len(f['uv'])!=2 or len(f['trims'])!=1 or math.dist(c['point'],f['point'])>1e-6 or abs(math.dist(f['normal'],[0.,0.,0.])-1.)>1e-10:raise ValueError('invalid supporting-face input receipt')
    return c
def failed_attempts(proof):
    rows=json.loads((proof/'attempts.json').read_bytes());paths=set()
    if len(rows)!=1:raise ValueError('failed edge capture inventory changed')
    for row in rows:
        log=proof/row['log'];paths.add(log)
        if (row['exit_status']!=1 or row['family']!='blend_curve_edge_pilot'
                or row['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_blend_curve_edge_pilot']
                or "'ObjRef' object has no attribute 'BrepEdge'"not in gzip.decompress(log.read_bytes()).decode()):raise ValueError('failed edge capture receipt changed')
        fixture=ROOT/'tools/rhino_oracle/fixtures/blend_curve_edge_pilot.json'
        if row['request_sha256']!=hashlib.sha256(fixture.read_bytes()).hexdigest():raise ValueError('failed edge request changed')
        for digest in row['sources'].values():
            p=ROOT/('tools/rhino_oracle/observations/source_snapshots/'+digest+'.gz')
            if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:raise ValueError('failed edge producer changed')
            paths.add(p)
    return paths
def collect(proof):
    rows,summary,paths=captures(proof)
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-symmetry-proof')
    paths.update(inherited);paths.add(ROOT/'docs/blend-curve-symmetry-validation.json');paths.update(failed_attempts(proof))
    workflows={};diagnostics={};picks=edges=surface_inputs=stations=derivatives=joins=cancelled=0
    for key,v in rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0];recipe=recipes.CASES[name];result=command_result(v)
        if v['prescribed_case']!=recipe or not v['returned_idle']:raise ValueError('edge recipe changed')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native edge geometry')
        if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('edge history changed')
        if name in DIAGNOSTICS:
            if name in recipes.EDGE_SELECTION:
                if 'Unknown command'not in v['history']or result!=('Cancel'if name=='edge_restricted_first'else'Nothing'):raise ValueError('edge selector diagnostic changed')
            elif name=='surface_cancel':
                if trace['errors']!=['incomplete owned BlendCrv input']or result!='Cancel':raise ValueError('edge cancellation diagnostic changed')
            elif (trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])
                    or 'Unknown command'in v['history']or result!='Nothing'or not v['history_changed']
                    or output(v)['curve']['degree']!=(7 if name.endswith('g3')else 9)):raise ValueError('curved high-order edge diagnostic changed')
            diagnostics[key]=DIAGNOSTICS[name];continue
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete edge workflow')
        accepted=result=='Nothing'
        if v['success']is not False or result not in ['Nothing','Cancel']or v['history_changed']is not accepted:raise ValueError('native edge result changed')
        o=output(v)
        if accepted:
            if o is None or len(o['samples'])!=33:raise ValueError('missing independent edge stations')
            if o['native_type']=='PolyCurve':joins+=1
            stations+=33
        else:
            cancelled+=1
            if o is not None or normalized(v['before'])!=normalized(v['after'])or v['pending_redo']is None:raise ValueError('edge cancellation changed document history')
        for before in v['before']:
            if 'brep'not in before:continue
            after=next(x for x in v['after']if x['attributes']['name']==before['attributes']['name'])
            if before!=after:raise ValueError('supporting surface parent changed')
        if accepted:
            if o['guide']:
                source=next(x for x in v['before']if x['attributes']['name']==o['attributes']['name'])
                if 'brep'in source or o['attributes']!=source['attributes']:raise ValueError('mixed edge join ownership changed')
            elif o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']:raise ValueError('edge output attributes changed')
            if any(x['selected']for x in v['after']):raise ValueError('edge acceptance selection changed')
        click_by_action={c['action']:c for c in trace.get('clicks',[])}
        for index,a in enumerate(recipe['actions']):
            if not isinstance(a,dict)or 'edge'not in a:continue
            nearest(click_by_action[index]);edges+=1
        surface_inputs+=sum(s for _,_,s in inputs(v))
        for c in click_by_action.values():
            a=recipe['actions'][c['action']]
            if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or c.get('shift_key_down')is not bool(a.get('shift',False)):raise ValueError('incomplete edge physical input')
            picks+=1
        for phase in ['before','after']:
            for obj in v[phase]:
                if 'curve'not in obj:continue
                samples=obj.get('derivative_samples',[])
                if len(samples)!=3 or any(len(s['derivatives'])!=5 or any(len(d)!=3 for d in s['derivatives'])for s in samples):raise ValueError('missing edge derivatives')
                derivatives+=3
        workflows[key]=v
    if (len(rows),len(workflows),len(diagnostics),cancelled)!=(48,41,7,1):raise ValueError('edge observation inventory changed')
    return workflows,diagnostics,summary,dict(physical_picks=picks,boundary_pick_receipts=edges,surface_inputs=surface_inputs,point_tangent_stations=stations,derivative_stations=derivatives,joined_workflows=joins,cancelled_workflows=cancelled),paths
