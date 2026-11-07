"""Audit public Shift receipts, endpoint constraints and length measurements."""
import math
from . import blend_curve_command as recipes,blend_curve_portions_evidence as prior
from .blend_curve_high_evidence import captures as read_captures,orders
from .blend_curve_evidence import command_result,normalized
from .arc_blend_radius_evidence import finite
from .check_blend_curve_symmetry import ROOT
BASELINE='59dfb119aee5dd0c089077edb16d85edeb51d290'
FAMILIES=dict(blend_curve_symmetry_pilot=list(recipes.SYMMETRY_PILOT),blend_curve_symmetry_discovery=list(recipes.SYMMETRY_DISCOVERY),blend_curve_symmetry_holdouts=list(recipes.SYMMETRY_HOLDOUTS),blend_curve_symmetry_lengths=list(recipes.SYMMETRY_LENGTHS))
INDEPENDENT={
 'symmetric_shape_baseline','symmetric_mixed_shape','symmetric_contact_start','symmetric_contact_end','symmetric_point_contact',
 'symmetric_mixed_g3_g4','symmetric_point_shape','symmetric_g0_peer','symmetric_mixed_reverse','symmetric_new_selection_independent',
}
def paired_case(name):return name.removesuffix('_lengths')not in INDEPENDENT
def captures(proof):return read_captures(proof,FAMILIES)
def handles(o):
    p=o['curve']['control_points'];return [math.dist(p[0]['point'],p[1]['point']),math.dist(p[-1]['point'],p[-2]['point'])]
def length_measurements(rows):
    count=0
    for key,v in rows.items():
        if not v['prescribed_case'].get('measure_lengths'):continue
        o=prior.output(v);measure=o.get('length_measurements',{});fractional=measure.get('fractional',[]);unit=measure.get('unit_domain_fractional',[])
        if ([p['tolerance']for p in fractional]!=[1e-6,1e-8,1e-10,1e-12]
                or [p['tolerance']for p in unit]!=[1e-6,1e-8,1e-10,1e-12]
                or not finite(measure)or o['curve']['domain'][0]!=0.
                or measure.get('default')!=o['curve']['domain'][1]
                or fractional[1]['length']!=measure['default']
                or any(a['length']!=b['length']for a,b in zip(fractional,unit))):raise ValueError('native fractional length measurement changed')
        if abs(fractional[-1]['length']-fractional[0]['length'])<=1e-5:raise ValueError('missing independent length convergence evidence')
        name=key.split('/')[1].removesuffix('_lengths')
        original=next(old for k,old in rows.items()if k.split('/')[1]==name)
        recipe=dict(v['prescribed_case']);recipe.pop('measure_lengths')
        if recipe!=original['prescribed_case']:raise ValueError('length measurement inputs changed')
        old=prior.output(original)['curve'];new=o['curve']
        if old['domain']!=new['domain']or old['knots']!=new['knots']or any(math.dist(a['point'],b['point'])>1e-12 or a['weight']!=b['weight']for a,b in zip(old['control_points'],new['control_points'])):raise ValueError('length measurement geometry changed')
        count+=1
    if count!=3:raise ValueError('length observation inventory changed')
    return count

def collect(proof):
    rows,summary,paths=captures(proof)
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-portions-proof')
    paths.update(inherited);paths.add(ROOT/'docs/blend-curve-portions-validation.json')
    picks=shift=stations=derivatives=joins=paired=0
    for key,v in rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0];recipe=recipes.CASES[name]
        if v['prescribed_case']!=recipe or not v['returned_idle']:raise ValueError('symmetry recipe changed')
        if not finite(v)or any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native symmetry geometry')
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete symmetry workflow')
        if v['success']is not False or command_result(v)!='Nothing'or not v['history_changed']:raise ValueError('native symmetry result changed')
        if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('symmetry history changed')
        output=prior.output(v)
        if output is None or len(output['samples'])!=33:raise ValueError('missing symmetry output or native stations')
        joined='Join=Yes'in recipe['actions']
        if joined!=(output['native_type']=='PolyCurve'):raise ValueError('symmetry join classification changed')
        b=prior.bindings(v);consumed=[n for n,t in b if n and not t]if joined else[]
        if consumed:
            source=next(x for x in v['before']if x['attributes']['name']==consumed[0])
            if output['attributes']!=source['attributes']or not output['guide']:raise ValueError('symmetry join ownership changed')
        elif output['attributes']['name']or output['attributes']['layer']!='Plane output'or output['attributes']['groups']or output['guide']:raise ValueError('symmetry output attributes changed')
        before=[x for x in v['before']if x['attributes']['name']not in consumed]
        if normalized(before)!=normalized([x for x in v['after']if x is not output]):raise ValueError('symmetry parent geometry changed')
        if not joined:
            order=orders(v)
            for i,(n,_)in enumerate(b):
                if n is None:order[i]=0
            if output['curve']['degree']!=sum(order)+1:raise ValueError('symmetry blend degree changed')
        if paired_case(name):
            paired+=1
            if not joined and name!='symmetric_then_independent'and abs(handles(output)[0]-handles(output)[1])>1e-10:raise ValueError('native paired handle lengths changed')
        if any(o['selected']for o in v['after']):raise ValueError('symmetry acceptance selection changed')
        for phase in ['before','after']:
            for o in v[phase]:
                if 'curve'not in o:continue
                samples=o.get('derivative_samples',[])
                if len(samples)!=3 or any(len(s['derivatives'])!=5 or any(len(d)!=3 for d in s['derivatives'])for s in samples):raise ValueError('missing symmetry derivative samples')
                derivatives+=3
        for click in trace.get('clicks',[]):
            requested=bool(recipe['actions'][click['action']].get('shift',False))
            if (not click['motion_acknowledged']or not click['click_acknowledged']or len(click['ray'])!=2
                    or click.get('shift_key_down')is not requested):raise ValueError('missing public Shift input receipt')
            picks+=1;shift+=requested
        stations+=33;joins+=joined
    length_measurements(rows)
    if (len(rows),picks,shift,stations,derivatives,joins,paired)!=(33,104,32,1089,483,3,23):raise ValueError('symmetry observation inventory changed: '+str((len(rows),picks,shift,stations,derivatives,joins,paired)))
    return rows,summary,picks,shift,stations,derivatives,joins,paired,paths
