"""Audit native same-parent Chamfer inputs, outcomes and immutable sources."""
import base64
import struct
import gzip
import json
from .fillet_getter_evidence import ROOT, HERE, finite, normalized, sha
from .chamfer_getter_evidence import result
from . import chamfer_command as recipes
FAMILIES = {
    'chamfer_self_' + name: list(getattr(recipes, 'SELF_' + name.upper()))
    for name in ['pilot', 'discovery', 'holdouts', 'spans']
}
BASELINE = '73ad5e503459e29a7f850f40a6dde35fa975a541'


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
                    if not obj['native_valid'] or ('curve' in obj and len(obj.get('samples', [])) != 33): raise ValueError('native geometry witnesses')
            if normalized(value['after']) != normalized(value['redo']) or normalized(value['before']) != normalized(value['undo']): raise ValueError('native history')
            if not value['history_changed'] and normalized(value['before']) != normalized(value['after']): raise ValueError('uncommitted model edit')
            if value['prescribed_case'].get('pending_redo') and not value['history_changed'] and value['pending_redo'] is None: raise ValueError('pending redo witness')
            if len(value['prescribed_case']['curves']) != 1: raise ValueError('same-parent source inventory')
            for shot in trace.get('viewport_snapshots', []):
                image = base64.b64decode(shot.get('png_base64', ''), validate=True)
                if not shot['captured'] or image[:8] != b'\x89PNG\r\n\x1a\n' or len(image) < 32 or struct.unpack('>II', image[16:24]) != (shot['width'], shot['height']): raise ValueError('viewport receipt')
            clicks = trace.get('clicks', [])
            if len(clicks) != sum(isinstance(a,dict) and 'curve' in a for a in value['prescribed_case']['actions']) or any(not c['motion_acknowledged'] or not c['click_acknowledged'] or len(c['ray']) != 2 or 'sight_point' not in c or c['curve'] != 0 for c in clicks): raise ValueError('public mouse receipts')
            cancels=[i for i,a in enumerate(value['prescribed_case']['actions']) if a=='Cancel']
            if value['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('physical Escape receipt')
            rows[family + '/' + row['id']] = value
    counts = dict(observations=len(rows), successful=sum(result(v) == 'Success' for v in rows.values()), cancelled=sum(result(v) == 'Cancel' for v in rows.values()), failed=sum(result(v) == 'Failure' for v in rows.values()), changed=sum(v['history_changed'] for v in rows.values()), images=sum(len(v['input_trace'][0].get('viewport_snapshots', [])) for v in rows.values()), physical_picks=sum(len(v['input_trace'][0]['clicks']) for v in rows.values()), point_tangent_stations=sum(len(o.get('samples', [])) for v in rows.values() for phase in ['before', 'after'] for o in v[phase]))
    if counts != dict(observations=46, successful=35, cancelled=1, failed=10, changed=35, images=0, physical_picks=91, point_tangent_stations=3861): raise ValueError('native coverage changed')
    return rows, captures, counts, paths
