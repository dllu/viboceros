"""Frozen public SDK captures isolating averaged G2 construction and holdouts."""
import gzip,json
from .surface_average_evidence import ROOT,HERE,sha,frozen,collect as previous
from .surface_match_evidence import collect as matching
from .fillet_getter_evidence import finite
from . import match_surface_average_cases as sdk
PROOF=ROOT/'docs/surface-average-curvature-proof'
FAMILIES={
    'match_surface_average_curvature_contexts':sdk.CURVATURE_CONTEXTS,
    'match_surface_average_curvature_rows':sdk.CURVATURE_ROWS,
    'match_surface_average_curvature_holdouts':sdk.CURVATURE_HOLDOUTS,
    'match_surface_average_curvature_nonuniform':sdk.CURVATURE_NONUNIFORM,
}


def collect(proof=PROOF):
    averages,pending,commands,paths=previous()
    curvature={k:v for k,v in matching()[3].items()if v['prescribed_case']['continuity']=='Curvature'}
    pilot=json.loads(gzip.decompress((HERE/'observations/match_surface_average_pilot.json.gz').read_bytes()))
    curvature.update({r['id']:r['value']for r in pilot['results']if r['value']['prescribed_case']['continuity']=='Curvature'})
    pending-=curvature.keys()
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('curvature capture inventory')
    paths.add(proof/'captures.json')
    for receipt,(family,recipes)in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/receipt['archive'];raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        stem=family.removeprefix('match_surface_average_').replace('_','-');log=proof/receipt['log']
        launch=['tools/rhino_oracle/run_headless.sh','exec','bash','-c','python3 -m tools.rhino_oracle.generate_'+family+
            ' > /tmp/match-average-'+stem+'.log 2>&1; capture_result=$?; echo "$capture_result" > /tmp/match-average-'+stem+'.status; exit "$capture_result"']
        if (receipt['command']!=launch or type(receipt['exit_status'])is not int or receipt['exit_status']!=0
                or receipt['archive']!='observations/'+family+'.json.gz' or receipt['observation_sha256']!=sha(raw)
                or receipt['request_sha256']!=sha(fixture.read_bytes()) or data['provenance']['request_sha256']!=receipt['request_sha256']
                or json.loads(fixture.read_bytes())!=sdk.request(list(recipes)) or data['engine']!='rhino' or data['iterations']!=1
                or [r['id']for r in data['results']]!=list(recipes)
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]):
            raise ValueError('curvature owned headless receipt')
        required={n+'.py'for n in ['generate_'+family,'match_surface_average_cases','match_surface_average_sdk',
            'match_surface_cases','match_surface_sdk','structural_sdk','rhino_worker','client','brep_join_probe','cap_probe','capture_family']}
        if set(data['provenance']['sources'])!=required:raise ValueError('curvature producer inventory')
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        for row in data['results']:
            key,v=row['id'],row['value']
            if (key in curvature or not finite(v) or v['prescribed_case']!=recipes[key] or v['success']is not True or v['source_unchanged']is not True
                    or v['output']is None or v['target']is None or len(v['inputs'])!=2):raise ValueError('curvature complete owned outputs')
            curvature[key]=v
    if (len(averages),len(curvature),len(pending),len(commands))!=(54,65,2,8):raise ValueError('curvature comparison scope')
    return averages,curvature,pending,commands,paths
