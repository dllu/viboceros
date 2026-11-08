"""Owned MatchSrf command traces, physical clicks and explicit diagnostic scope."""
import gzip,json
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
from .match_surface_command import PILOT,OPTIONS,QUALIFIED,request
PROOF=ROOT/'docs/match-surface-command-proof'
FAMILIES={ 'match_surface_command_pilot':PILOT,'match_surface_command_options':OPTIONS,
           'match_surface_command_qualified':QUALIFIED }
DIAGNOSTICS={'match_cmd_position','match_cmd_option_menus','match_cmd_tolerance_menu','match_cmd_tolerances_set'}


def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts]!=list(FAMILIES):raise ValueError('MatchSrf capture inventory')
    paths={proof/'captures.json',HERE/'run_headless.sh'};rows={};diagnostics={};captures=[]
    for receipt,(family,recipes) in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/receipt['archive']
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw);log=proof/receipt['log']
        if (type(receipt['exit_status']) is not int or receipt['exit_status']!=0
                or receipt['command'][:2]!=['tools/rhino_oracle/run_headless.sh','exec']
                or 'tools.rhino_oracle.generate_'+family not in ' '.join(receipt['command'])
                or receipt['archive']!='observations/'+family+'.json.gz'
                or receipt['observation_sha256']!=sha(raw)
                or receipt['request_sha256']!=sha(fixture.read_bytes())
                or data['provenance']['request_sha256']!=sha(fixture.read_bytes())
                or json.loads(fixture.read_bytes())!=request(list(recipes))
                or data['engine']!='rhino' or data['iterations']!=1
                or [r['id'] for r in data['results']]!=list(recipes)
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=[
                    'Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]):
            raise ValueError('MatchSrf capture receipt')
        required={name+'.py' for name in ['generate_'+family,'structural_sdk','rhino_worker','client',
            'catenary_point_picker','arc_point_picker','subcurve_direction_input','group_picking',
            'chamfer_point_picker','match_surface_point_picker','match_surface_command_probe',
            'match_surface_command','match_surface_cases','line_scene_probe','join_probe',
            'merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment',
            'number_token','capture_family']}
        if set(data['provenance']['sources'])!=required:raise ValueError('MatchSrf producer inventory')
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        captures.append(dict(family=family,observations=len(recipes),exit_status=0,engine_version=data['engine_version']))
        for r in data['results']:
            key,v=r['id'],r['value'];trace=v['input_trace'][0]
            if (not finite(v) or v['prescribed_case']!=recipes[key] or trace['completed']!=trace['expected']
                    or trace['expected']!=len(recipes[key]['actions'])
                    or trace['errors']!=(['owned MatchSrf input timeout'] if key=='match_cmd_option_menus' else [])
                    or len(trace['prompts'])!=trace['expected']
                    or len(trace.get('clicks',[]))!=sum(isinstance(a,dict) and 'pick'in a for a in recipes[key]['actions'])
                    or any(not c['motion_acknowledged'] or not c['click_acknowledged'] for c in trace.get('clicks',[]))
                    or not v['returned_idle']):raise ValueError('MatchSrf owned input completion')
            event=next(e for e in v['events'] if e['name']=='MatchSrf')
            if (event['result'] not in ['Success','Cancel'] or v['history_changed']!=(event['result']=='Success')
                    or v['success']!=(event['result']=='Success')
                    or any(not o['native_valid'] for stage in ['before','after','undo','redo'] for o in v[stage])):
                raise ValueError('MatchSrf native outcome')
            (diagnostics if key in DIAGNOSTICS else rows)[key]=v
    if (len(rows),len(diagnostics))!=(21,4):raise ValueError('MatchSrf scope inventory')
    return rows,diagnostics,captures,paths
