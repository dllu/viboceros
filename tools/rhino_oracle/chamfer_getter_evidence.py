"""Validate the complete native getters, frozen producers and failed attempts."""
import gzip
import json
from .fillet_getter_evidence import ROOT, HERE, sha, finite, normalized
from . import chamfer_command as recipes
BASELINE='bf8207b4ab47c1ed0bca09df84af7441a082cc31'
FAMILIES={'chamfer_getter_'+n:list(getattr(recipes,n.upper())) for n in ['pilot','discovery','holdouts']}
UNSUPPORTED={'chamfer_getter_holdouts/chamfer_oversized'}
def result(v):
    events=[e for e in v['events'] if e['name']=='Chamfer']
    if len(events)!=1: raise ValueError('native command result')
    return events[0]['result']
def collect(proof):
    paths=set(proof.glob('*'))|{HERE/'run_headless.sh'};rows={};captures=[]
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts]!=list(FAMILIES): raise ValueError('capture inventory')
    def frozen(sources):
        for digest in sources.values():
            p=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes()))!=digest: raise ValueError('frozen producer')
            paths.add(p)
    for receipt,(family,names) in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz');raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id'] for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names): raise ValueError('native input inventory')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()) or receipt['request_sha256']!=sha(fixture.read_bytes()) or receipt['observation_sha256']!=sha(raw): raise ValueError('capture hashes')
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]: raise ValueError('headless capture receipt')
        if gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]: raise ValueError('incomplete capture')
        paths.update([fixture,archive]);frozen(data['provenance']['sources'])
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            v=row['value'];trace=v['input_trace'][0];key=family+'/'+row['id']
            if not finite(v) or v['prescribed_case']!=recipes.CASES[row['id']] or not v['returned_idle'] or trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']): raise ValueError('incomplete native getter')
            if result(v) not in ['Success','Cancel'] or v['success'] is not (result(v)=='Success'): raise ValueError('native command result')
            for phase in ['before','after','undo','redo']:
                for o in v[phase]:
                    if not o['native_valid'] or ('curve' in o and len(o.get('samples',[]))!=33): raise ValueError('native geometry witnesses')
            if normalized(v['after'])!=normalized(v['redo']) or (v['history_changed'] and normalized(v['before'])!=normalized(v['undo'])): raise ValueError('native history')
            if not v['history_changed'] and normalized(v['before'])!=normalized(v['after']): raise ValueError('uncommitted edit')
            if v['prescribed_case'].get('pending_redo') and not v['history_changed'] and v['pending_redo'] is None: raise ValueError('pending redo witness')
            clicks=trace.get('clicks',[]);expected=sum(isinstance(a,dict) and 'curve' in a for a in v['prescribed_case']['actions'])
            if len(clicks)!=expected or any(not c['motion_acknowledged'] or not c['click_acknowledged'] or len(c['ray'])!=2 or 'sight_point' not in c for c in clicks): raise ValueError('public mouse receipts')
            cancel=[i for i,a in enumerate(v['prescribed_case']['actions']) if a=='Cancel']
            # The pilot predates client-side diagnostic export. Its original
            # driver, Escape input token and public EndCommand Cancel are frozen.
            if family.endswith('pilot'):
                if cancel and (result(v)!='Cancel' or not any(s['token']=='Escape' for s in trace['sent'])): raise ValueError('pilot Escape receipt')
            elif v['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancel]: raise ValueError('physical Escape receipt')
            rows[key]=v
    for name in ['cancel-token','sendkeys']:
        attempt=json.loads((proof/('failed-'+name+'.json')).read_bytes())
        if attempt['exit_status']!=1 or attempt['native_observations']!=0 or attempt['request_sha256']!=sha((HERE/'fixtures/chamfer_getter_pilot.json').read_bytes()) or not attempt['owned_pid'] or not attempt['settings_scheme'].startswith('VibocerosOracleChamfer'): raise ValueError('failed capture receipt')
        frozen(attempt['sources'])
    counts=dict(observations=len(rows),successful=sum(result(v)=='Success' for v in rows.values()),cancelled=sum(result(v)=='Cancel' for v in rows.values()),changed=sum(v['history_changed'] for v in rows.values()),physical_picks=sum(len(v['input_trace'][0].get('clicks',[])) for v in rows.values()),point_tangent_stations=sum(len(o.get('samples',[])) for v in rows.values() for phase in ['before','after'] for o in v[phase]))
    if (counts['observations'],counts['successful'],counts['cancelled'])!=(31,21,10): raise ValueError('coverage changed')
    return rows,captures,counts,paths
