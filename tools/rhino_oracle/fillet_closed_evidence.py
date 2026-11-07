"""Audit complete public Fillet captures and preserve failed attempts separately."""
import gzip
import json
from .fillet_getter_evidence import ROOT, HERE, finite, normalized, result, sha
from . import fillet_command as recipes
FAMILIES = dict(fillet_closed_pilot=list(recipes.CLOSED_PILOT), fillet_closed_final=list(recipes.CLOSED_FINAL))
BASELINE = '23b0cc92c8813ea29a396dd74fedbc118c9778eb'


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
            if result(value) not in ('Success', 'Cancel') or value['success'] is not (result(value) == 'Success') or 'Unknown command' in value['history']: raise ValueError('native command result')
            for phase in ['before', 'after', 'undo', 'redo']:
                for obj in value[phase]:
                    if not obj['native_valid'] or ('curve' in obj and len(obj.get('samples', [])) != 33): raise ValueError('native geometry witnesses')
            if normalized(value['after']) != normalized(value['redo']) or normalized(value['before']) != normalized(value['undo']): raise ValueError('native history')
            if not value['history_changed'] and (normalized(value['before']) != normalized(value['after']) or not value['prescribed_case'].get('pending_redo') or value['pending_redo'] is None): raise ValueError('cancel redo witness')
            clicks = trace.get('clicks', [])
            if len(clicks) != 2 or any(not c['motion_acknowledged'] or not c['click_acknowledged'] or len(c['ray']) != 2 or 'projected_point' not in c for c in clicks): raise ValueError('public mouse receipts')
            rows[family + '/' + row['id']] = value
    for name, message in [('failed-import', 'Attempted relative import'), ('failed-spatial', 'Rhino process exited before publishing a response')]:
        attempt = json.loads((proof / (name + '.json')).read_bytes())
        fixture = HERE / ('fixtures/' + attempt['family'] + '.json')
        log = gzip.decompress((proof / ('capture-' + name + '.log.gz')).read_bytes()).decode()
        if attempt['exit_status'] != 1 or attempt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + attempt['family']] or attempt['native_observations'] != 0 or attempt['request_sha256'] != sha(fixture.read_bytes()) or message not in log: raise ValueError('failed capture receipt')
        paths.add(fixture)
        for digest in attempt['sources'].values():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('failed frozen producer')
            paths.add(path)
    counts = dict(observations=len(rows), successful=sum(result(v) == 'Success' for v in rows.values()), cancelled=sum(result(v) == 'Cancel' for v in rows.values()), physical_picks=sum(len(v['input_trace'][0]['clicks']) for v in rows.values()), point_tangent_stations=sum(len(o.get('samples', [])) for v in rows.values() for phase in ['before', 'after'] for o in v[phase]))
    if {k:counts[k] for k in ['observations','successful','cancelled','physical_picks']} != dict(observations=16, successful=15, cancelled=1, physical_picks=32): raise ValueError('native coverage changed')
    return rows, captures, counts, paths
