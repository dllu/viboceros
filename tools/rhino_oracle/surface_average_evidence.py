"""Owned two-output geometry and physical MatchSrf averaging observations."""
import gzip,json
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .surface_match_evidence import collect as legacy
from .fillet_getter_evidence import finite
from . import match_surface_average_cases as sdk,match_surface_command as command
PROOF=ROOT/'docs/surface-average-proof'
FAMILIES={'match_surface_average_pilot':('sdk',sdk.PILOT),
          'match_surface_average_holdouts':('sdk',sdk.HOLDOUTS),
          'match_surface_average_command':('command',command.AVERAGE_WORKFLOWS)}
PENDING_COMMAND={'match_average_curvature_pending'}


def collect(proof=PROOF):
    _,_,_,old,_,paths=legacy();api=dict(old);commands={}
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('averaging capture inventory')
    paths.update([proof/'captures.json',proof/'attempts.json',HERE/'run_headless.sh']);producers={}
    for receipt,(family,(kind,recipes))in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/receipt['archive'];raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        log=proof/receipt['log'];stem=receipt['log'].removeprefix('capture-').removesuffix('.log.gz')
        launch=['tools/rhino_oracle/run_headless.sh','exec','bash','-c','python3 -m tools.rhino_oracle.generate_'+family+
            ' > /tmp/match-average-'+stem+'.log 2>&1; capture_result=$?; echo "$capture_result" > /tmp/match-average-'+stem+'.status; exit "$capture_result"']
        factory=sdk if kind=='sdk'else command
        if (receipt['kind']!=kind or type(receipt['exit_status'])is not int or receipt['exit_status']!=0 or receipt['command']!=launch
                or receipt['archive']!='observations/'+family+'.json.gz' or sha(raw)!=receipt['observation_sha256']
                or sha(fixture.read_bytes())!=receipt['request_sha256'] or data['provenance']['request_sha256']!=receipt['request_sha256']
                or json.loads(fixture.read_bytes())!=factory.request(list(recipes)) or data['engine']!='rhino' or data['iterations']!=1
                or [r['id']for r in data['results']]!=list(recipes)
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]):raise ValueError('averaging headless receipt')
        required={n+'.py'for n in ['generate_'+family,'match_surface_cases','structural_sdk','rhino_worker','client','capture_family']}
        required.update(n+'.py'for n in (['match_surface_average_cases','match_surface_average_sdk','match_surface_sdk','brep_join_probe','cap_probe']if kind=='sdk'else[
            'match_surface_command','match_surface_command_probe','match_surface_point_picker','arc_point_picker','catenary_point_picker',
            'chamfer_point_picker','subcurve_direction_input','group_picking','line_scene_probe','join_probe','merge_edges_probe',
            'conic_getter_input','managed_mouse_callback','snap_environment','number_token']))
        if set(data['provenance']['sources'])!=required:raise ValueError('averaging producer inventory')
        producers[family]=required
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        destination=api if kind=='sdk'else commands
        for row in data['results']:
            key,v=row['id'],row['value']
            if key in destination or not finite(v) or v['prescribed_case']!=recipes[key]:raise ValueError('averaging authored row')
            if kind=='sdk':
                if v['success']is not True or v['source_unchanged']is not True or v['output']is None or v['target']is None or len(v['inputs'])!=2:raise ValueError('incomplete averaging outputs')
            else:
                trace=v['input_trace'][0]
                if (trace['completed']!=len(recipes[key]['actions']) or trace['expected']!=trace['completed'] or trace['errors']
                        or len(trace['prompts'])!=trace['completed'] or sum(e['name']=='MatchSrf'for e in v['events'])!=1
                        or not v['returned_idle'] or len(trace['clicks'])!=2
                        or any(not c['motion_acknowledged'] or not c['click_acknowledged']for c in trace['clicks'])):raise ValueError('incomplete averaging command')
            destination[key]=v
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=1:raise ValueError('averaging failed-attempt inventory')
    for attempt in attempts:
        archive=HERE/attempt['archive'];raw=gzip.decompress(archive.read_bytes());value=json.loads(raw);log=proof/attempt['log']
        if (type(attempt['exit_status'])is not int or attempt['exit_status']!=1 or attempt['command']!=receipts[0]['command']
                or attempt['family']!=receipts[0]['family'] or value['family']!=attempt['family']
                or set(value['provenance']['sources'])!=producers[attempt['family']]
                or not archive.name.endswith('-'+sha(raw)+'.json.gz') or value['error']!=attempt['error']
                or 'unsupported oracle operation: average_surface_sdk'not in value['error']
                or value['provenance']['request_sha256']!=receipts[0]['request_sha256']
                or value['error']not in gzip.decompress(log.read_bytes()).decode()):raise ValueError('averaging failed dispatch receipt')
        frozen(value['provenance'],paths);paths.update([archive,log])
    qualified={k:v for k,v in api.items()if v['prescribed_case']['continuity']!='Curvature' and not v['prescribed_case']['refine']}
    pending=set(api)-set(qualified)
    if (len(api),len(qualified),len(pending),len(commands))!=(66,54,12,8):raise ValueError('averaging observation scope')
    return qualified,pending,commands,paths
