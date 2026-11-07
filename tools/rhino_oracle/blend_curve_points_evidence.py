"""Public point-endpoint input, ownership, history, and immutable capture evidence."""
import gzip
import hashlib
import json
from . import blend_curve_command as recipes, blend_curve_high_evidence as prior
from .blend_curve_evidence import command_result, output, normalized
from .arc_blend_radius_evidence import finite, HERE
from .check_blend_curve_points import ROOT
BASELINE='0e50ae9d7724b6086f629cc6cc98d894a677c885'
FAMILIES=dict(blend_curve_point_pilot=list(recipes.POINT_PILOT),
              blend_curve_point_discovery=list(recipes.POINT_DISCOVERY),
              blend_curve_point_holdouts=list(recipes.POINT_HOLDOUTS),
              blend_curve_point_ownership=list(recipes.POINT_OWNERSHIP),
              blend_curve_point_modes=list(recipes.POINT_MODES),
              blend_curve_point_pick_context=list(recipes.POINT_PICK_CONTEXT))

def captures(proof): return prior.captures(proof,FAMILIES)

def point_index(v):
    actions=v['prescribed_case']['actions']
    if 'Point' not in actions:return None
    return sum(isinstance(a,dict) and 'curve' in a for a in actions[:actions.index('Point')])

def initial_pick_point(v):
    prompt=v['input_trace'][0]['prompts'][0]
    if not any('BlendStart='+mode in prompt for mode in ['CurveEnd','PickPoint']):
        raise ValueError('missing initial point selection state')
    return 'BlendStart=PickPoint' in prompt

def joined_sources(v):
    # Join fixtures use prescribed lines. Public pre-output sight points classify
    # their contacts; no output coordinates or control points seed this decision.
    pick_point=initial_pick_point(v);ends=0;result=[]
    trim='Trim=Yes' in v['prescribed_case']['actions']
    for index,action in enumerate(v['prescribed_case']['actions']):
        if isinstance(action,str) and action.startswith('BlendStart='):
            pick_point=action.endswith('PickPoint')
        elif action=='Point' and ends<2:ends+=1
        elif isinstance(action,dict) and 'curve' in action and ends<2:
            mode='PickPoint' if pick_point else 'CurveEnd'
            if 'BlendStart='+mode not in v['input_trace'][0]['prompts'][index]:
                raise ValueError('native source selection mode changed')
            connected=True
            if pick_point and not trim:
                curve=v['prescribed_case']['curves'][action['curve']]
                if curve['type']!='line':raise ValueError('unqualified joined source class')
                point=next(c['sight_point'] for c in v['input_trace'][0]['clicks'] if c['action']==index)
                connected=any(sum((a-b)**2 for a,b in zip(point,p))<=1e-16 for p in [curve['start'],curve['end']])
            if connected:result.append('guide'+str(action['curve']))
            ends+=1
    return result

def calibration_signature(v,click):
    return (v['prescribed_case']['curves'],v['prescribed_case']['view'],v['construction_plane'],
            v['model_tolerance'],click['curve'],click['pixel'],click['ray'])

def calibrations(rows):
    measured=[]
    for key,v in rows.items():
        if not key.startswith('blend_curve_point_pick_context/'):continue
        for click in v['input_trace'][0]['clicks']:
            context=click.get('pick_context',{})
            references=context.get('references',[])
            if (context.get('success') is not True or context.get('mode')!='Wireframe'
                    or context.get('style')!='PointPick' or len(references)!=1
                    or references[0]['selection_method']!=1 or len(references[0]['point'])!=3
                    or context['parameter']!=references[0]['parameter']
                    or context['point']!=references[0]['point']):
                raise ValueError('invalid public pick-context input')
            source=v['prescribed_case']['curves'][click['curve']]
            degree=source['degree'];controls=source['control_points']
            if source['type']!='nurbs' or len(controls)!=degree+1:
                raise ValueError('unqualified calibration source')
            a,b=context['domain'];t=(references[0]['parameter']-a)/(b-a)
            if not 0<=t<=1:raise ValueError('calibration parameter outside source')
            net=[[p['point'][i]*p['weight'] for i in range(3)]+[p['weight']] for p in controls]
            while len(net)>1:net=[[(1-t)*a+t*b for a,b in zip(p,q)] for p,q in zip(net,net[1:])]
            evaluated=[x/net[0][3] for x in net[0][:3]]
            if max(abs(a-b) for a,b in zip(evaluated,references[0]['point']))>1e-12:
                raise ValueError('calibrated point is not on prescribed source')
            measured.append((calibration_signature(v,click),references[0]['point']))
    original=rows['blend_curve_point_holdouts/point_reverse_spatial_g4']
    click=original['input_trace'][0]['clicks'][0]
    matches=[point for signature,point in measured if signature==calibration_signature(original,click)]
    if len(measured)!=3 or not matches or any(p!=matches[0] for p in matches):
        raise ValueError('missing exact spatial input calibration')
    return 3,1

def collect(proof):
    rows,summary,paths=captures(proof)
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-high-proof')
    paths.update(inherited);paths.add(ROOT/'docs/blend-curve-high-validation.json')
    matched,diagnostics={},{}
    picks=stations=derivatives=joins=cancelled=0
    for key,v in rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0]
        if v['prescribed_case']!=recipes.CASES[name] or not v['returned_idle']:
            raise ValueError('point recipe changed')
        if not finite(v) or any(not o['native_valid'] for phase in ['before','after','undo','redo'] for o in v[phase]):
            raise ValueError('invalid native point geometry')
        if name=='point_two_points':
            if (command_result(v)!='Failure' or trace['errors']!=['incomplete owned BlendCrv input']
                    or trace['completed']!=4 or trace['expected']!=7 or v['history_changed']
                    or normalized(v['before'])!=normalized(v['after']) or output(v) is not None
                    or ' Point ' in trace['prompts'][2]):
                raise ValueError('two-point selector diagnostic changed')
            diagnostics[key]='Point is absent after choosing one point; the attempted second Point sequence ended Failure with incomplete input.'
            continue
        if trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(recipes.CASES[name]['actions']) or 'Unknown command' in v['history']:
            raise ValueError('incomplete point workflow')
        accepted='cancel' not in name
        if v['success'] is not False or command_result(v)!=('Nothing' if accepted else 'Cancel'):
            raise ValueError('native point result semantics changed')
        if accepted:
            o=output(v)
            if not v['history_changed'] or o is None or len(o['samples'])!=33:
                raise ValueError('missing native point output or stations')
            if normalized(v['after'])!=normalized(v['redo']) or normalized(v['before'])!=normalized(v['undo']):
                raise ValueError('point history changed')
            orders=prior.orders(v)
            if point_index(v) is not None:orders[point_index(v)]=0
            if o['native_type']=='PolyCurve':
                consumed=joined_sources(v)
                unaffected=lambda objects:[x for x in objects if x['attributes']['name'] not in consumed]
                after_others=[x for x in v['after'] if x is not o]
                if consumed:
                    source=next(x for x in v['before'] if x['attributes']['name']==consumed[0])
                    if o['attributes']!=source['attributes'] or not o['guide']:
                        raise ValueError('point join ownership changed')
                elif o['attributes']['name'] or o['attributes']['layer']!='Plane output' or o['attributes']['groups'] or o['guide']:
                    raise ValueError('point join ownership changed')
                if normalized(unaffected(v['before']))!=normalized(after_others):
                    raise ValueError('point join ownership changed')
                joins+=1
            elif o['curve']['degree']!=sum(orders)+1 or o['attributes']['layer']!='Plane output' or o['attributes']['groups']:
                raise ValueError('point blend degree or output attributes changed')
            stations+=33
        else:
            if v['history_changed'] or normalized(v['before'])!=normalized(v['after']) or not v['pending_redo']:
                raise ValueError('point cancellation lost history')
            cancelled+=1
        if v['prescribed_case'].get('measure_jets'):
            for phase in ['before','after']:
                for o in v[phase]:
                    if 'curve' not in o:continue
                    samples=o.get('derivative_samples',[])
                    if len(samples)!=3 or any(len(s['derivatives'])!=5 or any(len(d)!=3 for d in s['derivatives']) for s in samples):
                        raise ValueError('missing point derivative samples')
                    derivatives+=3
        for click in trace.get('clicks',[]):
            if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray'])!=2:
                raise ValueError('incomplete point physical input')
            picks+=1
        matched[key]=v
    calibrations(rows)
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=1:raise ValueError('point failed-attempt inventory changed')
    attempt=attempts[0]
    log=gzip.decompress((proof/attempt['log']).read_bytes()).decode()
    worker=gzip.decompress((proof/attempt['worker_log']).read_bytes()).decode()
    if (attempt['family']!='blend_curve_point_discovery' or attempt['exit_status']!=130
            or attempt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_blend_curve_point_discovery']
            or 'KeyboardInterrupt' not in log or 'operation point_start_g1: start' not in worker
            or 'worker: response published' in worker or not (proof/attempt['screenshot']).exists()
            or attempt['request_sha256']!=hashlib.sha256((HERE/'fixtures/blend_curve_point_discovery.json').read_bytes()).hexdigest()):
        raise ValueError('missing interrupted point capture evidence')
    for digest in attempt['sources'].values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:
            raise ValueError('failed point producer changed')
        paths.add(p)
    if (len(matched),len(diagnostics),stations,derivatives,joins,cancelled)!=(41,1,1254,441,12,3):
        raise ValueError('point qualification inventory changed: '+str((len(matched),len(diagnostics),stations,derivatives,joins,cancelled)))
    return matched,diagnostics,summary,picks,stations,derivatives,joins,cancelled,paths
