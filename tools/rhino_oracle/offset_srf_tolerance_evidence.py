"""Retain every native attempt and distinguish incomplete/queued input pilots."""
import gzip
import json
from collections import Counter
from . import offset_surface_command as recipes
from .offset_surface_tolerance_cases import TOLERANCE_PILOT,TOLERANCE_HOLDOUTS,TOLERANCE_FINAL
from .offset_srf_loose_evidence import authored_equal
from .offset_surface_getter_evidence import frozen,result
from .fillet_getter_evidence import HERE,finite,normalized,sha
from .fillet_zero_history import audit
from .check_offset_srf_tolerance import PROOF

BASELINE='dc437499c0d557133ac734aee84b15bd0dfe4e70'
FAMILIES={'offset_srf_tolerance_'+name:list(cases)for name,cases in [
    ('pilot',TOLERANCE_PILOT),('holdouts',TOLERANCE_HOLDOUTS),('final',TOLERANCE_FINAL)]}
DIAGNOSTICS={
    'offset_tp_getter_cancel':'Two Escapes were sent; the redundant second key may have entered the following command.',
    'offset_tp_getter_keep':'The following command received no owned input after the two-Escape pilot; a fresh session repeats Enter retention.',
}

def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory')
    rows={};captures=[];paths={proof/'captures.json',HERE/'run_headless.sh'};counts=Counter()
    for receipt,(family,names)in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz')
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names):raise ValueError('native input inventory')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()) or receipt['request_sha256']!=sha(fixture.read_bytes()) or receipt['observation_sha256']!=sha(raw):raise ValueError('capture hashes')
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]:raise ValueError('headless capture receipt')
        log=proof/receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('capture log')
        paths.update([fixture,archive,log]);frozen(data['provenance']['sources'],paths)
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            key=row['id'];v=row['value'];trace=v['input_trace'][0];status=result(v)
            if key in rows or not authored_equal(v['prescribed_case'],recipes.CASES[key]) or not finite(v) or not v['returned_idle'] or v['success']is not(status=='Success'):raise ValueError('native recipe or result')
            rows[key]=v;counts['observations']+=1;counts[status]+=1
            history=audit(v,'OffsetSrf')
            if history['recorded']is not(status=='Success') or normalized(v['before'])!=normalized(v['undo']) or normalized(v['after'])!=normalized(v['redo']):raise ValueError('native history geometry')
            if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native geometry')
            if key not in DIAGNOSTICS and(trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']) or 'Unknown command:'in v['history']):raise ValueError('incomplete native input')
            counts['diagnostics'if key in DIAGNOSTICS else'qualified']+=1
            cancels=[i for i,a in enumerate(v['prescribed_case']['actions'][:trace['completed']])if a=='Cancel']
            if v.get('external_keys')!=[dict(action=i,key='Escape',private_owned_window=True)for i in cancels]:raise ValueError('owned cancellation receipt')
    if dict(counts)!=dict(observations=30,Success=24,Cancel=6,qualified=28,diagnostics=2):raise ValueError('native coverage counts: '+str(counts))
    if rows['offset_tp_getter_cancel']['input_trace'][0]['errors'] or rows['offset_tp_getter_keep']['input_trace'][0]['completed']!=0 or not rows['offset_tp_getter_keep']['input_trace'][0]['errors']:raise ValueError('diagnostic classification')
    return rows,captures,dict(counts),paths
