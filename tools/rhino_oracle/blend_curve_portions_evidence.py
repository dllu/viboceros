"""Audit frozen temporary-source inputs, native geometry and parent ownership."""
import gzip
import hashlib
import json
from . import blend_curve_command as recipes,blend_curve_points_evidence as prior
from .blend_curve_high_evidence import captures as read_captures,orders
from .blend_curve_evidence import command_result,normalized
from .arc_blend_radius_evidence import finite
from .check_blend_curve_portions import ROOT
BASELINE='8ef8ddcbc3fc18a7a26e1123a6d67c1d7605a7dc'
FAMILIES=dict(blend_curve_portion_pilot=list(recipes.PORTION_PILOT),blend_curve_portion_discovery=list(recipes.PORTION_DISCOVERY),blend_curve_portion_holdouts=list(recipes.PORTION_HOLDOUTS))
def captures(proof):return read_captures(proof,FAMILIES)
def output(v):
    # A preserved closed/polycurve parent can precede the newly joined output.
    # Match output by changed document identity, never by the parent's class.
    before={o['attributes']['name']:o for o in v['before']}
    candidates=[o for o in v['after']if 'curve'in o and (o['attributes']['name']not in before or normalized([o])!=normalized([before[o['attributes']['name']]]))]
    unnamed=[o for o in candidates if not o['attributes']['name']]
    return unnamed[0]if unnamed else next((o for o in candidates if o['native_type']=='PolyCurve'),None)

def bindings(v):
    """Classify the accepted sources from getter actions, independently of output."""
    result=[];pending=None;point=False
    for a in v['prescribed_case']['actions']:
        if a=='SubCrv':pending=dict(source=None,start=False,length=False,locked=False);continue
        if a=='Cancel':
            if pending is not None:pending=None;continue
            break
        if pending is not None:
            p=pending
            if isinstance(a,dict)and 'curve'in a:p['source']='guide'+str(a['curve']);continue
            if isinstance(a,dict)and 'hover'in a:continue
            if isinstance(a,str)and a.startswith('Direction='):p['locked']=a.endswith('Locked');continue
            if type(a)in [int,float]or isinstance(a,dict)and 'number'in a:
                if a==0:pending=None;continue
                p['length']=True
                if not p['locked']:continue
            elif isinstance(a,list)or isinstance(a,dict)and 'click'in a:
                if not p['start']:p['start']=True;continue
            else:raise ValueError('unclassified SubCrv input')
            result.append((p['source'],True));pending=None;continue
        if a=='Point':point=True
        elif point and isinstance(a,list):result.append((None,False));point=False
        elif isinstance(a,dict)and 'curve'in a and len(result)<2:result.append(('guide'+str(a['curve']),False))
    return result

def collect(proof):
    rows,summary,paths=captures(proof)
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-points-proof')
    paths.update(inherited);paths.add(ROOT/'docs/blend-curve-points-validation.json')
    picks=stations=derivatives=joins=cancelled=0
    for key,v in rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0];actions=recipes.CASES[name]['actions']
        if v['prescribed_case']!=recipes.CASES[name]or not v['returned_idle']:raise ValueError('portion recipe changed')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native portion geometry')
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(actions)or 'Unknown command'in v['history']:raise ValueError('incomplete portion workflow')
        accepted=name!='portion_cancel'
        if v['success']is not False or command_result(v)!=('Nothing'if accepted else'Cancel'):raise ValueError('native portion result changed')
        if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('portion history changed')
        if accepted:
            o=output(v);b=bindings(v)
            if len(b)!=2 or not v['history_changed']or o is None or len(o['samples'])!=33:raise ValueError('missing portion output or source bindings')
            joined='Join=Yes'in actions
            if (o['native_type']=='PolyCurve')!=joined:raise ValueError('portion join classification changed')
            consumed=[name for name,temp in b if name and not temp]if joined else []
            before={x['attributes']['name']:x for x in v['before']}
            if consumed:
                if o['attributes']!=before[consumed[0]]['attributes']or not o['guide']:raise ValueError('portion join ownership changed')
            elif o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']or o['guide']:raise ValueError('portion output attributes changed')
            if joined:
                remaining=[x for x in v['before']if x['attributes']['name']not in consumed]
                if normalized(remaining)!=normalized([x for x in v['after']if x is not o]):raise ValueError('temporary parent changed or whole source survived join')
                joins+=1
            else:
                # Only whole inputs can be trimmed. Private portions always retain
                # their parents, including a whole/portion pair from one parent.
                mutable={name for name,temp in b if name and not temp}if 'Trim=Yes'in actions else set()
                for original in v['before']:
                    name=original['attributes']['name']
                    if name in mutable:continue
                    actual=next(x for x in v['after']if x['attributes']['name']==name)
                    if normalized([original])!=normalized([actual]):raise ValueError('temporary portion parent changed')
                order=orders(v)
                for i,(name,_)in enumerate(b):
                    if name is None:order[i]=0
                if o['curve']['degree']!=sum(order)+1:raise ValueError('portion blend degree changed')
            if any(x['selected']for x in v['after']):raise ValueError('portion acceptance selection changed')
            stations+=33
        else:
            if v['history_changed']or normalized(v['before'])!=normalized(v['after'])or not v['pending_redo']:raise ValueError('portion cancellation lost pending redo')
            cancelled+=1
        if v['prescribed_case'].get('measure_jets'):
            for phase in ['before','after']:
                for o in v[phase]:
                    if 'curve'not in o:continue
                    samples=o.get('derivative_samples',[])
                    if len(samples)!=3 or any(len(s['derivatives'])!=5 or any(len(d)!=3 for d in s['derivatives'])for s in samples):raise ValueError('missing portion derivative samples')
                    derivatives+=3
        for click in trace.get('clicks',[]):
            if not click['motion_acknowledged']or not click['click_acknowledged']or len(click['ray'])!=2:raise ValueError('incomplete portion physical input')
            picks+=1
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if [a['exit_status']for a in attempts]!=[1,1]:raise ValueError('portion attempt inventory changed')
    for a,needle in zip(attempts,['multiple values','only one ** allowed']):
        if needle not in gzip.decompress((proof/a['log']).read_bytes()).decode():raise ValueError('missing portion failed-attempt receipt')
        if a['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_blend_curve_portion_discovery']:raise ValueError('portion attempt command changed')
        fixture=ROOT/'tools/rhino_oracle/fixtures/blend_curve_portion_discovery.json'
        if a.get('request_sha256',a.get('intended_request_sha256'))!=hashlib.sha256(fixture.read_bytes()).hexdigest():raise ValueError('portion attempt request changed')
        for digest in a['sources'].values():
            p=ROOT/('tools/rhino_oracle/observations/source_snapshots/'+digest+'.gz')
            if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:raise ValueError('portion failed producer changed')
            paths.add(p)
    if len(rows)!=36 or cancelled!=1:raise ValueError('portion observation inventory changed')
    return rows,summary,picks,stations,derivatives,joins,cancelled,paths
