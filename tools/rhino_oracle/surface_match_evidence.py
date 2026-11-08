"""Immutable owned surface-matching SDK evidence and explicit scope limits."""
import gzip
import json
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
from .match_surface_cases import PILOT,HOLDOUTS,FINAL,REFINEMENT,ORIENTED,request
PROOF=ROOT/'docs/surface-match-proof'
FAMILIES=dict(match_surface_pilot=PILOT,match_surface_holdouts=HOLDOUTS,
    match_surface_final=FINAL,match_surface_refinement=REFINEMENT,match_surface_oriented=ORIENTED)


def collect(proof=PROOF):
    paths={proof/'captures.json',HERE/'run_headless.sh'}
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts]!=list(FAMILIES):raise ValueError('surface-match capture inventory')
    rows,diagnostics,audits,averages,captures={},{},{},{},[]
    for receipt,(family,recipes) in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz')
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        expected_status=None if family=='match_surface_final' else 0
        if (json.loads(fixture.read_bytes())!=request(list(recipes)) or data['engine']!='rhino'
                or data['iterations']!=1 or [r['id'] for r in data['results']]!=list(recipes)
                or type(receipt['exit_status']) is not type(expected_status) or receipt['exit_status']!=expected_status
                or receipt['archive']!='observations/'+family+'.json.gz'
                or receipt['observation_sha256']!=sha(raw)
                or data['provenance']['request_sha256']!=sha(fixture.read_bytes())
                or receipt['request_sha256']!=sha(fixture.read_bytes())
                or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m',
                                        'tools.rhino_oracle.generate_'+family]):
            raise ValueError('surface-match headless capture receipt')
        log=proof/receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines()!=[
                'Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]:
            raise ValueError('surface-match capture log')
        required={n+'.py' for n in ['generate_'+family,'match_surface_cases','match_surface_sdk',
            'structural_sdk','rhino_worker','client','brep_join_probe','cap_probe','capture_family']}
        if set(data['provenance']['sources'])!=required:raise ValueError('surface-match frozen producer inventory')
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        captures.append(dict(family=family,observations=len(recipes),exit_status=expected_status,
            engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            key,v=row['id'],row['value']
            if (v['prescribed_case']!=recipes[key] or not finite(v) or v['success'] is not True
                    or v['source_unchanged'] is not True or v['output'] is None
                    or (v['target'] is not None)!=recipes[key]['average']):
                raise ValueError('surface-match authored native result')
            if family=='match_surface_final':diagnostics[key]=v
            elif family=='match_surface_refinement':audits[key]=v
            elif recipes[key]['average']:averages[key]=v
            else:rows[key]=v
    if (len(rows),len(diagnostics),len(audits),len(averages))!=(66,16,9,6):raise ValueError('surface-match scope inventory')
    # The supplemental launch has no exit receipt. Its successful immutable
    # protocol is retained and checked against a new independent owned launch.
    for key,value in diagnostics.items():
        repeat=rows[key.replace('match_f_','match_v_')]
        if value!=repeat:raise ValueError('supplemental/repeated native observations differ')
    for value in audits.values():
        samples=value['audit']['samples'];settings=value['audit']['settings']
        if not settings['enabled'] or not samples:raise ValueError('native refinement audit')
        if value['prescribed_case']['continuity']!='Position' and max(s['angle'] for s in samples)<=settings['angle']:
            raise ValueError('lost measured refinement tolerance excess')
    return rows,diagnostics,audits,averages,captures,paths
