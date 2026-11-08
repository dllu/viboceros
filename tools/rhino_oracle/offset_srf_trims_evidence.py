"""Retained source recipes, failed import and completed private-display capture."""
import gzip
import json
from collections import Counter
from . import offset_srf_tolerance_evidence as previous
from .check_offset_srf_trims import PROOF
from .offset_surface_trim_cases import TRIM_OFFSETS,TRIM_FINAL
from .offset_surface_getter_evidence import frozen, result
from .fillet_getter_evidence import HERE, finite, normalized, sha
from .fillet_zero_history import audit

FAMILIES={'offset_srf_trims':TRIM_OFFSETS,'offset_srf_trims_final':TRIM_FINAL}
DIAGNOSTICS=dict(previous.DIAGNOSTICS,offset_te_delete_postselect=
    'DeleteInput was sent while hidden with Solid=No; Rhino ignored it. A fresh capture sets it while Solid=Yes before offsetting an open sheet.')

def collect(proof=PROOF):
    rows,captures,counts,paths=previous.collect()
    counts=Counter(counts)
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [(r['family'],r['exit_status']) for r in receipts]!=[('offset_srf_trims',1),('offset_srf_trims',0),('offset_srf_trims_final',0)]: raise ValueError('capture attempts')
    paths.update([proof/'captures.json',HERE/'run_headless.sh'])
    from .offset_surface_command import request
    for receipt in receipts:
        family=receipt['family']; recipes=FAMILIES[family]
        fixture=HERE/('fixtures/'+family+'.json')
        if json.loads(fixture.read_bytes())!=request(list(recipes)): raise ValueError('native input inventory')
        paths.add(fixture)
        if receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]: raise ValueError('headless capture receipt')
        archive=HERE/receipt['archive']; raw=gzip.decompress(archive.read_bytes()); data=json.loads(raw)
        provenance=data['provenance']
        if provenance['request_sha256']!=sha(fixture.read_bytes()) or receipt['observation_sha256']!=sha(raw): raise ValueError('capture hashes')
        paths.add(archive); frozen(provenance['sources'],paths)
        log=proof/receipt['log']; text=gzip.decompress(log.read_bytes()).decode(); paths.add(log)
        if receipt['exit_status']:
            if receipt['exit_status']!=1 or data['family']!=family or data['response']['results']!=[] or "unexpected token '*'" not in data['error'] or data['error'] not in text: raise ValueError('failed import evidence')
            counts['failed_attempts']+=1
            continue
        if text.splitlines()!=['Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]: raise ValueError('capture log')
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id'] for r in data['results']]!=list(recipes): raise ValueError('native result inventory')
        captures.append(dict(family=family,observations=len(recipes),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            key=row['id']; v=row['value']; trace=v['input_trace'][0]; status=result(v)
            if key in rows or not previous.authored_equal(v['prescribed_case'],recipes[key]) or not finite(v) or not v['returned_idle'] or v['success']is not(status=='Success'): raise ValueError('native recipe or result')
            if trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']) or (key not in DIAGNOSTICS and 'Unknown command:' in v['history']): raise ValueError('incomplete native input')
            if audit(v,'OffsetSrf')['recorded']is not(status=='Success') or normalized(v['before'])!=normalized(v['undo']) or normalized(v['after'])!=normalized(v['redo']): raise ValueError('native history geometry')
            if any(not o['native_valid'] for phase in ['before','after','undo','redo'] for o in v[phase]): raise ValueError('invalid native geometry')
            cancels=[i for i,a in enumerate(v['prescribed_case']['actions']) if a=='Cancel']
            if v['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('owned cancellation receipt')
            rows[key]=v; counts['observations']+=1; counts[status]+=1; counts['diagnostics' if key in DIAGNOSTICS else 'qualified']+=1
    if 'Unknown command:' not in rows['offset_te_delete_postselect']['history']: raise ValueError('diagnostic classification')
    if dict(counts)!=dict(observations=39,Success=32,Cancel=7,qualified=36,diagnostics=3,failed_attempts=1): raise ValueError('native coverage counts: '+str(counts))
    return rows,captures,dict(counts),paths
