"""Audit native distinct closed-source Chamfer inputs, outcomes and immutable sources."""
import base64
import struct
import gzip
import json
from .fillet_getter_evidence import ROOT, HERE, finite, normalized, sha
from .chamfer_getter_evidence import result
from . import chamfer_command as recipes
FAMILIES = {
    'chamfer_closed_' + name: list(getattr(recipes, 'CLOSED_' + {'verification':'FINAL','choices':'BRANCHES'}.get(name,name.upper())))
    for name in ['pilot', 'verification', 'choices']
}
BASELINE = 'c93bee7ead1a5472aec338213fc89b5e06a9e941'
INVALID={'chamfer_closed_verification/closed_two_circles_joined_zero','chamfer_closed_choices/closed_consumed_line'}


def collect(proof):
    paths = set(proof.glob('*')) | {HERE / 'run_headless.sh'}
    rows, captures = {}, []
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES): raise ValueError('capture inventory')
    for receipt, (family, names) in zip(receipts, FAMILIES.items()):
        fixture = HERE / ('fixtures/' + family + '.json'); archive = HERE / ('observations/' + family + '.json.gz')
        raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
        if data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != names or json.loads(fixture.read_bytes()) != recipes.request(names): raise ValueError('native input inventory')
        if data['provenance']['request_sha256'] != sha(fixture.read_bytes()) or receipt['request_sha256'] != sha(fixture.read_bytes()) or receipt['observation_sha256'] != sha(raw): raise ValueError('capture hashes')
        if receipt['exit_status'] != 0 or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + family]: raise ValueError('headless capture receipt')
        if gzip.decompress((proof / receipt['log']).read_bytes()).decode().splitlines() != ['Capturing ' + str(len(names)) + ' cases: ' + family, 'Recorded ' + family]: raise ValueError('incomplete capture')
        paths.update([fixture, archive]); sources = data['provenance']['sources']
        for digest in sources.values():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('frozen producer')
            paths.add(path)
        captures.append(dict(family=family, observations=len(names), engine_version=data['engine_version'], raw_sha256=sha(raw)))
        for row in data['results']:
            value = row['value']; trace = value['input_trace'][0]
            if not finite(value) or value['prescribed_case'] != recipes.CASES[row['id']] or not value['returned_idle'] or trace['errors'] or trace['completed'] != trace['expected'] or trace['expected'] != len(value['prescribed_case']['actions']): raise ValueError('incomplete native getter')
            if result(value) not in ('Success', 'Cancel', 'Failure') or value['success'] is not (result(value) == 'Success') or 'Unknown command' in value['history']: raise ValueError('native command result')
            for phase in ['before', 'after', 'undo', 'redo']:
                for obj in value[phase]:
                    if not obj['native_valid']:
                        if family+'/'+row['id'] not in INVALID or not obj.get('invalid_geometry') or 'curve' in obj: raise ValueError('unexpected invalid native geometry')
                    elif 'curve' in obj and len(obj.get('samples',[]))!=33: raise ValueError('native geometry witnesses')
            if normalized(value['after']) != normalized(value['redo']) or normalized(value['before']) != normalized(value['undo']): raise ValueError('native history')
            if not value['history_changed'] and normalized(value['before']) != normalized(value['after']): raise ValueError('uncommitted model edit')
            if value['prescribed_case'].get('pending_redo') and not value['history_changed'] and value['pending_redo'] is None: raise ValueError('pending redo witness')
            if len(value['prescribed_case']['curves']) != 2: raise ValueError('closed-source inventory')
            for shot in trace.get('viewport_snapshots', []):
                image = base64.b64decode(shot.get('png_base64', ''), validate=True)
                if not shot['captured'] or image[:8] != b'\x89PNG\r\n\x1a\n' or len(image) < 32 or struct.unpack('>II', image[16:24]) != (shot['width'], shot['height']): raise ValueError('viewport receipt')
            clicks = trace.get('clicks', [])
            if len(clicks) != sum(isinstance(a,dict) and 'curve' in a and 'pick' in a for a in value['prescribed_case']['actions']) or any(not c['motion_acknowledged'] or not c['click_acknowledged'] or len(c['ray']) != 2 or 'sight_point' not in c for c in clicks): raise ValueError('public mouse receipts')
            if 'source_seed_points' in value['prescribed_case']:
                if row['id']!='closed_coincident_circles' or len(clicks)!=0: raise ValueError('typed source inventory')
                sent=trace['sent'][-2:]
                if not all(s['token'].startswith("'_-SelID ") and ('Select '+which+' curve') in s['prompt'] for s,which in zip(sent,['first','second'])): raise ValueError('typed source receipts')
            cancels=[i for i,a in enumerate(value['prescribed_case']['actions']) if a=='Cancel']
            if value['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('physical Escape receipt')
            rows[family + '/' + row['id']] = value
    counts = dict(observations=len(rows), successful=sum(result(v) == 'Success' for v in rows.values()), cancelled=sum(result(v) == 'Cancel' for v in rows.values()), failed=sum(result(v) == 'Failure' for v in rows.values()), changed=sum(v['history_changed'] for v in rows.values()), images=sum(len(v['input_trace'][0].get('viewport_snapshots', [])) for v in rows.values()), physical_picks=sum(len(v['input_trace'][0].get('clicks',[])) for v in rows.values()), point_tangent_stations=sum(len(o.get('samples', [])) for v in rows.values() for phase in ['before', 'after'] for o in v[phase]))
    if counts != dict(observations=33, successful=30, cancelled=1, failed=2, changed=29, images=0, physical_picks=63, point_tangent_stations=4950): raise ValueError('native coverage changed')
    for name in ['holdouts','final','branches']:
        attempt=json.loads((proof/('failed-'+name+'.json')).read_bytes());fixture=HERE/('fixtures/'+attempt['family']+'.json')
        if attempt['exit_status']!=1 or attempt['native_observations']!=0 or attempt['request_sha256']!=sha(fixture.read_bytes()): raise ValueError('failed capture receipt')
        paths.add(fixture)
        for digest in attempt['sources'].values():
            p=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes()))!=digest: raise ValueError('failed producer')
            paths.add(p)
    invalid={key for key,v in rows.items() if any(not o['native_valid'] for o in v['after'])}
    if invalid!=INVALID: raise ValueError('invalid native result inventory')
    return rows, captures, counts, paths
