"""All native loose offset attempts, including unknown-option discoveries."""
import gzip
import json
import math
import sys
from collections import Counter
from . import offset_surface_command as recipes
from .offset_surface_loose_cases import LOOSE_PILOT,LOOSE_HOLDOUTS,LOOSE_FINAL,LOOSE_CLOSED,LOOSE_SHELLS
from .offset_surface_loose_trim_cases import LOOSE_TRIMS
from .offset_surface_getter_evidence import frozen,result
from .fillet_getter_evidence import HERE,finite,normalized,sha
from .fillet_zero_history import audit
from .check_offset_srf_loose import PROOF

BASELINE='06ebff919408c1fb82f9813c2d386e245a8d1a6d'
FAMILIES={'offset_srf_loose_'+name:list(cases)for name,cases in [
    ('pilot',LOOSE_PILOT),('holdouts',LOOSE_HOLDOUTS),('final',LOOSE_FINAL),
    ('trims',LOOSE_TRIMS),('closed',LOOSE_CLOSED),('shells',LOOSE_SHELLS)]}
DIAGNOSTICS={
    'offset_loose_polysurface':'Loose and DeleteInput were unavailable on this polysurface; it ran an ordinary offset.',
    'offset_l_periodic_uv':'DeleteInput was unavailable on the closed input; a clean recipe follows.',
    'offset_l_sphere_poles':'DeleteInput was unavailable on the closed input; a clean recipe follows.',
    'offset_l_crease':'Rhino stored the creased surface as a polysurface and did not expose Loose.',
}

def authored_equal(a,b):
    # Frozen recipes call libm for the torus controls. IronPython/x86 and
    # CPython/ARM can differ by a few ulps; no observed output is substituted.
    if isinstance(a,dict)and isinstance(b,dict):return set(a)==set(b)and all(authored_equal(a[k],b[k])for k in a)
    if isinstance(a,list)and isinstance(b,list):return len(a)==len(b)and all(authored_equal(x,y)for x,y in zip(a,b))
    if type(a)in(int,float)and type(b)in(int,float):return math.isfinite(a)and math.isfinite(b)and abs(a-b)<=8*sys.float_info.epsilon*max(1.,abs(a),abs(b))
    return type(a)is type(b)and a==b

def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory')
    rows={};captures=[];paths={proof/'captures.json',HERE/'run_headless.sh'};counts=Counter()
    for receipt,(family,names) in zip(receipts,FAMILIES.items()):
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
            if key in rows or not authored_equal(v['prescribed_case'],recipes.CASES[key]) or not finite(v) or not v['returned_idle'] or v['success'] is not(status=='Success'):raise ValueError('native recipe or result')
            rows[key]=v;counts['observations']+=1;counts[status]+=1
            history=audit(v,'OffsetSrf')
            if history['recorded'] is not(status=='Success') or normalized(v['before'])!=normalized(v['undo']) or normalized(v['after'])!=normalized(v['redo']):raise ValueError('native history geometry')
            if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid native geometry')
            if trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']):raise ValueError('incomplete native input')
            if ('Unknown command:'in v['history']) is not(key in DIAGNOSTICS):raise ValueError('diagnostic classification')
            counts['diagnostics'if key in DIAGNOSTICS else'qualified']+=1
            cancels=[i for i,a in enumerate(v['prescribed_case']['actions'])if a=='Cancel']
            if v.get('external_keys')!=[dict(action=i,key='Escape',private_owned_window=True)for i in cancels]:raise ValueError('owned cancellation receipt')
    if dict(counts)!=dict(observations=48,Success=44,Cancel=4,qualified=44,diagnostics=4):raise ValueError('native coverage counts')
    failures=sorted((HERE/'observations/failures').glob('offset_srf_loose_*.json.gz'))
    if len(failures)!=1:raise ValueError('failed attempt inventory')
    failure=json.loads(gzip.decompress(failures[0].read_bytes()));response=failure['response']
    if failure['family']!='offset_srf_loose_holdouts' or response['results'] or 'exactly three coordinates'not in response['error']:raise ValueError('failed source receipt')
    if failure['provenance']['request_sha256']!=sha((HERE/'fixtures/offset_srf_loose_holdouts.json').read_bytes()):raise ValueError('failed request receipt')
    frozen(failure['provenance']['sources'],paths);paths.update([failures[0],proof/'invalid-uv-source.log.gz'])
    if b'exactly three coordinates'not in gzip.decompress((proof/'invalid-uv-source.log.gz').read_bytes()):raise ValueError('failed capture log')
    return rows,captures,dict(counts),paths
