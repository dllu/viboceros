"""Immutable corrected MatchSrf enum-menu capture; tolerance rows are discovery."""
import gzip,json
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
from .match_surface_command import TOLERANCE_GETTERS,request
PROOF=ROOT/'docs/match-surface-menu-proof'
FAMILY='match_surface_command_tolerance_getters'


def collect(proof=PROOF):
    receipt=json.loads((proof/'capture.json').read_bytes());fixture=HERE/('fixtures/'+FAMILY+'.json')
    archive=HERE/('observations/'+FAMILY+'.json.gz');raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
    log=proof/'capture.log.gz';paths={proof/'capture.json',fixture,archive,log,HERE/'run_headless.sh'}
    if (receipt['family']!=FAMILY or type(receipt['exit_status']) is not int or receipt['exit_status']!=0
            or receipt['command'][:2]!=['tools/rhino_oracle/run_headless.sh','exec']
            or 'tools.rhino_oracle.generate_'+FAMILY not in ' '.join(receipt['command'])
            or receipt['observation_sha256']!=sha(raw) or receipt['request_sha256']!=sha(fixture.read_bytes())
            or data['provenance']['request_sha256']!=sha(fixture.read_bytes())
            or json.loads(fixture.read_bytes())!=request(list(TOLERANCE_GETTERS))
            or gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing 9 cases: '+FAMILY,'Recorded '+FAMILY]
            or data['engine']!='rhino' or data['iterations']!=1
            or [r['id'] for r in data['results']]!=list(TOLERANCE_GETTERS)
            or receipt['qualified_rows']!=['match_tol_complete_menus'] or receipt['tolerance_discovery_rows']!=8):
        raise ValueError('MatchSrf menu capture receipt')
    required={name+'.py' for name in ['generate_'+FAMILY,'structural_sdk','rhino_worker','client',
        'catenary_point_picker','arc_point_picker','subcurve_direction_input','group_picking',
        'chamfer_point_picker','match_surface_point_picker','match_surface_command_probe',
        'match_surface_command','match_surface_cases','line_scene_probe','join_probe',
        'merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token','capture_family']}
    if set(data['provenance']['sources'])!=required:raise ValueError('MatchSrf menu producer inventory')
    frozen(data['provenance'],paths)
    rows={}
    for r in data['results']:
        key,v=r['id'],r['value'];trace=v['input_trace'][0];recipe=TOLERANCE_GETTERS[key]
        if (not finite(v) or v['prescribed_case']!=recipe or trace['errors']
                or trace['completed']!=len(recipe['actions']) or trace['expected']!=trace['completed']
                or len(trace['prompts'])!=trace['expected'] or not v['returned_idle']
                or len(trace['clicks'])!=2 or any(not c['motion_acknowledged'] or not c['click_acknowledged'] for c in trace['clicks'])):
            raise ValueError('MatchSrf menu owned input')
        rows[key]=v
    value=rows['match_tol_complete_menus'];trace=value['input_trace'][0]
    if (not value['success'] or not value['history_changed'] or
            'PreserveOtherEnd <Position> ( Nothing  Position  Tangency  Curvature )' not in trace['prompts']
            or 'IsocurveDirection <Automatic> ( Automatic  MatchTarget  Perpendicular  Keep )' not in trace['prompts']):
        raise ValueError('MatchSrf corrected menu outcome')
    return value,paths
