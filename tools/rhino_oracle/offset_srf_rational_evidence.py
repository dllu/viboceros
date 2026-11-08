"""Frozen rational-offset inputs, owned headless captures and actual history."""
import gzip
import json
from collections import Counter
from pathlib import Path
from .offset_surface_command import request, CASES
from .offset_surface_rational_cases import RATIONAL_PILOT, RATIONAL_HOLDOUTS
from .offset_surface_rational_final_cases import RATIONAL_FINAL
from .offset_srf_loose_evidence import authored_equal
from .offset_surface_getter_evidence import frozen, result
from .fillet_getter_evidence import HERE, finite, normalized, sha
from .fillet_zero_history import audit

PROOF=Path(__file__).resolve().parents[2]/'docs/offset-srf-rational-proof'
FAMILIES={'offset_srf_rational_pilot':RATIONAL_PILOT,
          'offset_srf_rational_holdouts':RATIONAL_HOLDOUTS,
          'offset_srf_rational_final':RATIONAL_FINAL}

def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory')
    rows={};captures=[];counts=Counter();paths={proof/'captures.json',HERE/'run_headless.sh'}
    for receipt,(family,recipes) in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz')
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        if json.loads(fixture.read_bytes())!=request(list(recipes)) or data['engine']!='rhino' or data['iterations']!=1 or [r['id'] for r in data['results']]!=list(recipes):raise ValueError('native input inventory')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()) or receipt['request_sha256']!=sha(fixture.read_bytes()) or receipt['observation_sha256']!=sha(raw):raise ValueError('capture hashes')
        if receipt['exit_status']!=0 or receipt['archive']!='observations/'+family+'.json.gz' or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]:raise ValueError('headless capture receipt')
        log=proof/receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]:raise ValueError('capture log')
        paths.update([fixture,archive,log]);frozen(data['provenance']['sources'],paths)
        captures.append(dict(family=family,observations=len(recipes),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            key=row['id'];v=row['value'];trace=v['input_trace'][0];status=result(v)
            if key in rows or not authored_equal(v['prescribed_case'],CASES[key]) or not finite(v) or not v['returned_idle'] or v['success']is not(status=='Success'):raise ValueError('native recipe or result')
            if trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']) or 'Unknown command:'in v['history']:raise ValueError('incomplete native input')
            if audit(v,'OffsetSrf')['recorded']is not(status=='Success') or normalized(v['before'])!=normalized(v['undo']) or normalized(v['after'])!=normalized(v['redo']):raise ValueError('native history geometry')
            if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native geometry')
            cancels=[i for i,a in enumerate(v['prescribed_case']['actions'])if a=='Cancel']
            if v['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True)for i in cancels]:raise ValueError('owned cancellation receipt')
            rows[key]=v;counts['observations']+=1;counts[status]+=1
    if dict(counts)!=dict(observations=9,Success=8,Cancel=1):raise ValueError('native coverage counts')
    return rows,captures,dict(counts),paths
