"""Audit open Chamfer contacts, exact inputs, actual history and invalid joins."""
import gzip
import json
from . import chamfer_command as recipes
from .chamfer_getter_evidence import result
from .fillet_getter_evidence import ROOT, HERE, normalized, sha
from .fillet_zero_evidence import collect_captures, difference
from .fillet_zero_history import audit

BASELINE = '3fdba777495feded56cf60c2a72f55608784437c'
FAMILIES = {'chamfer_open_'+name:list(getattr(recipes,'OPEN_'+name.upper()))
            for name in ['pilot','holdouts','curves','choices','overlaps']}
INVALID = {'chamfer_open_pilot/open_chamfer_oversized_joined',
           'chamfer_open_curves/open_chamfer_crossing_oversized_joined',
           'chamfer_open_curves/open_chamfer_collinear_smooth_joined',
           'chamfer_open_curves/open_chamfer_nurbs_consumed_line_joined'}
EXPECTED_COUNTS = dict(observations=69,successful=62,cancelled=1,failed=6,changed=61,
                       invalid_outputs=4,images=0,physical_picks=137,point_tangent_stations=9603)
EQUAL_RECORDS = {'chamfer_open_pilot/open_chamfer_zero_parts',
                 'chamfer_open_holdouts/open_chamfer_zero_nurbs'}


def invalid_layout(obj):
    domain = obj.get('curve_domain'); ends = obj.get('curve_endpoints')
    return isinstance(domain,list) and len(domain) == 2 and domain[1] > domain[0] and isinstance(ends,list) and len(ends) == 2 and all(isinstance(p,list) and len(p) == 3 for p in ends)


def legacy(rows, paths):
    records = []
    for family,name,modern in [('chamfer_getter_discovery','chamfer_zero_parts','open_chamfer_zero_parts'),
                               ('chamfer_getter_holdouts','chamfer_oversized','open_chamfer_oversized_parts')]:
        path = HERE/('observations/'+family+'.json.gz'); data = json.loads(gzip.decompress(path.read_bytes()))
        old = next(r['value'] for r in data['results'] if r['id'] == name)
        new = rows['chamfer_open_pilot/'+modern]
        if old['prescribed_case']['curves'] != new['prescribed_case']['curves']: raise ValueError('legacy source correspondence')
        maximum = max(difference(normalized(old[p]),normalized(new[p])) for p in ['before','after'])
        if maximum > 1e-6: raise ValueError('legacy geometry repeat')
        if name == 'chamfer_zero_parts':
            prompt = old['input_trace'][0]['prompts'][-1]
            if old['history_changed'] is not False or not all(token in prompt for token in ['Distances=0,0','Join=No','Trim=Yes']) or audit(new,'Chamfer') != dict(recorded=True,geometry_changed=False): raise ValueError('legacy history correction')
        records.append(dict(id=family+'/'+name,witness='chamfer_open_pilot/'+modern,
                            maximum_scene_difference=maximum,old_inferred_history=old['history_changed'],verified_history=new['history_changed']))
        paths.add(path)
        for digest in data['provenance']['sources'].values():
            path = HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('legacy frozen producer')
            paths.add(path)
    return records


def collect(proof):
    receipts = json.loads((proof/'captures.json').read_bytes())
    rows,captures,counts,paths = collect_captures(proof,receipts,FAMILIES,INVALID,EXPECTED_COUNTS,
        recipes_module=recipes,result_fn=result,invalid_validator=invalid_layout,cancel_token='Escape')
    histories = {key:audit(value,'Chamfer') for key,value in rows.items()}
    if {key for key,h in histories.items() if h['recorded'] and not h['geometry_changed']} != EQUAL_RECORDS: raise ValueError('equal-geometry history inventory')
    for value in rows.values():
        cancels = [i for i,a in enumerate(value['prescribed_case']['actions']) if a == 'Cancel']
        if value.get('external_keys',[]) != [dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('owned cancellation receipt')
    near = rows['chamfer_open_holdouts/open_chamfer_oversized_near']
    if near['prescribed_case']['actions'][1] != 3.9999999 or 'Distances=4,1 ' not in near['input_trace'][0]['prompts'][-1] or not any(s['action'] == 1 and float(s['token']) == 3.9999999 for s in near['input_trace'][0]['sent']): raise ValueError('exact numeric input receipt')
    counts.update(geometry_changed=sum(h['geometry_changed'] for h in histories.values()),unchanged_geometry_records=len(EQUAL_RECORDS))
    return rows,captures,counts,paths,legacy(rows,paths)
