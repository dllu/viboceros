"""Audit curved-face higher-order workflows and independent partials."""
import gzip
import copy
import hashlib
import json
import math
from . import blend_curve_command as recipes,blend_curve_edges_evidence as prior
from .blend_curve_high_evidence import captures as read_captures
from .blend_curve_evidence import command_result,normalized,output
from .blend_curve_surface_policy import characterize,contacts,frame,dot
from .arc_blend_radius_evidence import finite
from .blend_curve_length_audit import measure
from .check_blend_curve_surface_high import ROOT
BASELINE='d4f6a346911e04ded96868a981408733704bbae4'
FAMILIES=dict(blend_curve_surface_high_discovery=list(recipes.SURFACE_HIGH_DISCOVERY),blend_curve_surface_high_holdouts=list(recipes.SURFACE_HIGH_HOLDOUTS),blend_curve_surface_high_planarity=list(recipes.SURFACE_HIGH_PLANARITY))
def recipe_matches(actual,expected):
    if actual==expected:return True
    # IronPython and CPython round the boundary recipe's weighted sums a few
    # ulps apart. Only derived source-pick coordinates may differ; surfaces,
    # options, view, action kinds and every other input remain exact.
    actual=copy.deepcopy(actual)
    if len(actual.get('actions',[]))!=len(expected.get('actions',[])):return False
    for a,b in zip(actual['actions'],expected['actions']):
        if not isinstance(a,dict)or not isinstance(b,dict)or not all(k in a and k in b for k in ['curve','edge','pick']):continue
        if len(a['pick'])!=3 or len(b['pick'])!=3:return False
        if any(not math.isfinite(x)or not math.isfinite(y)or abs(x-y)>8*max(math.ulp(x),math.ulp(y))for x,y in zip(a['pick'],b['pick'])):return False
        a['pick']=b['pick']
    return actual==expected
def captures(proof):
    result=read_captures(proof,FAMILIES)
    for receipt in json.loads((proof/'captures.json').read_bytes()):
        family=receipt['family'];base=ROOT/'tools/rhino_oracle'
        raw=gzip.decompress((base/('observations/'+family+'.json.gz')).read_bytes())
        request=(base/('fixtures/'+family+'.json')).read_bytes()
        if hashlib.sha256(raw).hexdigest()!=receipt['observation_sha256']or hashlib.sha256(request).hexdigest()!=receipt['request_sha256']:raise ValueError('surface capture digest changed')
    return result
def promoted():
    rows,_,_=prior.captures(ROOT/'docs/blend-curve-edges-proof')
    return {k:v for k,v in rows.items()if k.split('/')[1]in ['surface_curved_g3','surface_curved_g4']}
def fourth_policy(rows):
    key='blend_curve_surface_high_holdouts/surface_high_fourth_g4';v=rows[key];_,face=frame(v,3)
    data=face['fourth_order'];p=data['partials'];third=face['third_order']['partials'];n=face['normal'];speed=math.sqrt(dot(p[1],p[1]))
    # This prescribed section has linear X and zero tangential second/third
    # partials at its boundary; the normalized fourth normal target is direct.
    tangent=[x/speed for x in p[1]]
    if (not data['success']or len(p)!=14 or abs(dot(p[4],tangent))>1e-10
            or abs(dot(third[8],tangent))>1e-10):raise ValueError('fourth-order source diagnostic changed')
    source=dot(p[13],n)/speed**4
    d=output(v)['derivative_samples'][0]['derivatives'];speed=math.sqrt(dot(d[1],d[1]));blend=math.sqrt(dot(d[4],d[4]))/speed**4
    if abs(source)<.5 or blend>1e-8:raise ValueError('native surface zero-fourth policy changed')
    return dict(id=key,source_normalized_fourth_normal=source,blend_normalized_fourth_norm=blend)
def attempts(proof):
    rows=json.loads((proof/'attempts.json').read_bytes());paths=set()
    if len(rows)!=1:raise ValueError('surface capture attempt inventory changed')
    row=rows[0]
    if row['exit_status']!=130 or row['family']!='blend_curve_surface_high_holdouts'or row['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_blend_curve_surface_high_holdouts']:raise ValueError('surface capture attempt terminal receipt changed')
    for field in ['log','progress','screenshot']:paths.add(proof/row[field])
    if 'KeyboardInterrupt'not in gzip.decompress((proof/row['log']).read_bytes()).decode()or not (proof/row['screenshot']).read_bytes().startswith(b'\x89PNG'):raise ValueError('surface menu attempt evidence changed')
    fixture=ROOT/'tools/rhino_oracle/fixtures/blend_curve_surface_high_holdouts.json'
    if hashlib.sha256(fixture.read_bytes()).hexdigest()!=row['request_sha256']:raise ValueError('surface menu attempt request changed')
    for digest in row['sources'].values():
        p=ROOT/('tools/rhino_oracle/observations/source_snapshots/'+digest+'.gz')
        if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:raise ValueError('surface menu attempt producer changed')
        paths.add(p)
    validation=json.loads((proof/'validation-attempts.json').read_bytes())
    if len(validation)!=2:raise ValueError('planarity regression inventory changed')
    for row,message in zip(validation,['blend_curve_surface_high_planarity/surface_high_near_plane_g2: normalized derivative error 2.8799999999999993e-3','Native desired fractional length precision: 6.279723315856245e-4, ratio=1.5927260008942607e1']):
        if row['status']!=101 or row['command']!=['cargo','test','-p','viboceros-command','--test','blend_curve_surface_high','--','--nocapture']:raise ValueError('planarity regression receipt changed')
        p=proof/row['log'];log=gzip.decompress(p.read_bytes()).decode()
        if message not in log or 'test result: FAILED. 0 passed; 1 failed;'not in log:raise ValueError('missing prior planarity regression failure')
        paths.add(p)
    lint=json.loads((proof/'lint-attempts.json').read_bytes())
    if len(lint)!=1 or lint[0]['status']!=101 or lint[0]['name']!='clippy-portable':raise ValueError('lint attempt receipt changed')
    p=proof/lint[0]['log']
    if 'error: comparison to empty slice'not in gzip.decompress(p.read_bytes()).decode():raise ValueError('missing initial lint diagnostic')
    paths.add(p)
    return paths
def length_diagnostics(rows,proof):
    audit={key.split('/')[1]:measure(output(v)['curve'])for key,v in rows.items()if key.startswith('blend_curve_surface_high_planarity/')}
    if len(audit)!=3 or audit!=json.loads((proof/'planarity-lengths.json').read_bytes()):raise ValueError('independent long-handle length audit changed')
    if any(abs(row['native_domain_minus_length'])<=max(2e-6,1e-8*row['native_domain_span'])for row in audit.values()):raise ValueError('lost observed long-handle domain mismatch')
    return {'blend_curve_surface_high_planarity/'+key:row for key,row in audit.items()}
def collect(proof):
    new,summary,paths=captures(proof);rows=promoted()|new
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-edges-proof')
    paths.update(inherited);paths.add(ROOT/'docs/blend-curve-edges-validation.json');paths.update(attempts(proof))
    picks=stations=derivatives=partial_stations=joins=cancelled=0
    for key,v in rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0];recipe=recipes.CASES[name]
        if not recipe_matches(v['prescribed_case'],recipe)or not v['returned_idle']:raise ValueError('higher surface recipe changed')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native higher surface geometry')
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete higher surface workflow')
        accepted=command_result(v)=='Nothing'
        if v['success']is not False or v['history_changed']is not accepted or command_result(v)not in ['Nothing','Cancel']:raise ValueError('higher surface native result changed')
        if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('higher surface history changed')
        o=output(v)
        if accepted:
            if o is None or len(o['samples'])!=33:raise ValueError('missing higher surface stations')
            joins+=o['native_type']=='PolyCurve';stations+=33
            if any(x['selected']for x in v['after']):raise ValueError('higher surface acceptance selection changed')
            if o['guide']:
                source=next(x for x in v['before']if x['attributes']['name']==o['attributes']['name'])
                if 'brep'in source or o['attributes']!=source['attributes']:raise ValueError('higher surface join ownership changed')
            elif o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']:raise ValueError('higher surface output attributes changed')
        else:
            cancelled+=1
            if o is not None or normalized(v['before'])!=normalized(v['after'])or v['pending_redo']is None:raise ValueError('higher surface cancellation changed document history')
        for before in v['before']:
            if 'brep'not in before:continue
            after=next(x for x in v['after']if x['attributes']['name']==before['attributes']['name'])
            if before!=after:raise ValueError('higher surface parent changed')
        for c in trace['clicks']:
            a=recipe['actions'][c['action']]
            if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or c.get('shift_key_down')is not bool(a.get('shift',False)):raise ValueError('missing higher surface physical input')
            picks+=1
            if 'edge_pick_context'not in c:continue
            prior.nearest(c)
            if key not in new:continue
            for candidate in c['edge_pick_context']['candidates']:
                for face in candidate['faces']:
                    p=face.get('third_order',{})
                    if not p.get('success')or len(p.get('point',[]))!=3 or len(p.get('partials',[]))!=9 or any(len(d)!=3 for d in p['partials']):raise ValueError('missing independent third surface partials')
                    if key.startswith('blend_curve_surface_high_holdouts/'):
                        p=face.get('fourth_order',{})
                        if not p.get('success')or len(p.get('partials',[]))!=14:raise ValueError('missing independent fourth surface diagnostic')
                    partial_stations+=1
        for phase in ['before','after']:
            for x in v[phase]:
                if 'curve'not in x:continue
                s=x.get('derivative_samples',[])
                if len(s)!=3 or any(len(q['derivatives'])!=5 for q in s):raise ValueError('missing higher surface blend derivatives')
                derivatives+=3
    policy=characterize(new);fourth=fourth_policy(new);length_diagnostics(new,proof)
    observed=json.loads((proof/'construction-policy.json').read_bytes())
    if observed!=dict(third_order=policy,fourth_order=fourth):raise ValueError('surface construction characterization changed')
    if (len(new),len(rows),len(promoted()),joins,cancelled)!=(40,42,2,2,1):raise ValueError('higher surface inventory changed')
    return rows,new,summary,dict(physical_picks=picks,point_tangent_stations=stations,derivative_stations=derivatives,surface_partial_stations=partial_stations,joined_workflows=joins,cancelled_workflows=cancelled),paths
