"""Audit zero-radius Fillet inputs, outcomes, invalid controls and immutable sources."""
import base64
import struct
import gzip
import json
from .fillet_getter_evidence import ROOT, HERE, finite, normalized, sha
from .fillet_getter_evidence import result
from . import fillet_command as recipes
FAMILIES = {'fillet_zero_'+name:list(getattr(recipes,'ZERO_'+name.upper())) for name in ['pilot','holdouts','verification']}
BASELINE = 'd6e139375f495f24d9a17ddcad686ccbd6955df0'
EXPECTED_COUNTS = {'observations': 40, 'successful': 35, 'cancelled': 1, 'failed': 4, 'changed': 29, 'invalid_outputs': 2, 'images': 0, 'physical_picks': 80, 'point_tangent_stations': 5280}

INVALID = {'fillet_zero_pilot/zero_two_circles_joined', 'fillet_zero_holdouts/zero_two_circles_joined_reverse'}

def collect_captures(proof, receipts, families, invalid, expected_counts):
    paths = set(proof.glob('*')) | {HERE / 'run_headless.sh'}
    rows, captures = {}, []
    if [r['family'] for r in receipts] != list(families): raise ValueError('capture inventory')
    for receipt, (family, names) in zip(receipts, families.items()):
        fixture = HERE / ('fixtures/' + family + '.json'); archive = HERE / ('observations/' + family + '.json.gz')
        raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
        if data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != names or json.loads(fixture.read_bytes()) != recipes.request(names): raise ValueError('native input inventory')
        if data['provenance']['request_sha256'] != sha(fixture.read_bytes()) or receipt['request_sha256'] != sha(fixture.read_bytes()) or receipt['observation_sha256'] != sha(raw): raise ValueError('capture hashes')
        if receipt['exit_status'] != 0 or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + family]: raise ValueError('headless capture receipt')
        lines = gzip.decompress((proof / receipt['log']).read_bytes()).decode().splitlines()
        if lines != ['Capturing ' + str(len(names)) + ' cases: ' + family, 'Recorded ' + family]: raise ValueError('incomplete capture')
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
                        key = family+'/'+row['id']
                        if key not in invalid or phase not in ['after','redo'] or obj['native_type'] != 'PolyCurve' or obj.get('invalid_geometry') is not True or obj.get('curve_domain') is not None or obj.get('curve_endpoints') != [None,None] or 'curve' in obj: raise ValueError('unexpected invalid native geometry')
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
            cancels=[i for i,a in enumerate(value['prescribed_case']['actions']) if a=='Cancel']
            if [s['action'] for s in trace['sent'] if s['token']=='_Cancel'] != cancels: raise ValueError('cancellation token receipt')
            rows[family + '/' + row['id']] = value
    counts = dict(observations=len(rows), successful=sum(result(v) == 'Success' for v in rows.values()), cancelled=sum(result(v) == 'Cancel' for v in rows.values()), failed=sum(result(v) == 'Failure' for v in rows.values()), changed=sum(v['history_changed'] for v in rows.values()), invalid_outputs=sum(not o['native_valid'] for v in rows.values() for o in v['after']), images=sum(len(v['input_trace'][0].get('viewport_snapshots', [])) for v in rows.values()), physical_picks=sum(len(v['input_trace'][0].get('clicks',[])) for v in rows.values()), point_tangent_stations=sum(len(o.get('samples', [])) for v in rows.values() for phase in ['before', 'after'] for o in v[phase]))
    if {key for key,v in rows.items() if any(not o['native_valid']for o in v['after'])} != invalid: raise ValueError('invalid control inventory')
    if counts != expected_counts: raise ValueError('native coverage changed')
    return rows, captures, counts, paths


def collect_discovery(proof):
    return collect_captures(proof,json.loads((proof/'captures.json').read_bytes()),FAMILIES,INVALID,EXPECTED_COUNTS)


FINAL_INVALID = {'fillet_zero_final/verified_'+key.split('/',1)[1] for key in INVALID}
FINAL_COUNTS = dict(EXPECTED_COUNTS, changed=30)


def difference(a, b):
    if type(a) in (int,float) and type(b) in (int,float): return abs(a-b)
    if type(a) is not type(b): raise ValueError('native repeat structure')
    if isinstance(a,dict):
        if set(a)!=set(b): raise ValueError('native repeat fields')
        return max((difference(a[k],b[k]) for k in a),default=0.)
    if isinstance(a,list):
        if len(a)!=len(b): raise ValueError('native repeat count')
        return max((difference(x,y)for x,y in zip(a,b)),default=0.)
    if a!=b: raise ValueError('native repeat value')
    return 0.


def collect(proof):
    from .fillet_zero_history import audit
    from .fillet_zero_controls import collect as controls
    discovery, captures, _, paths = collect_discovery(proof)
    receipt = json.loads((proof/'final-capture.json').read_bytes())
    rows, final_captures, counts, extra = collect_captures(proof,[receipt],{'fillet_zero_final':list(recipes.ZERO_FINAL)},FINAL_INVALID,FINAL_COUNTS)
    paths.update(extra);maximum=0.;unchanged_records=[]
    for key,v in rows.items():
        h=audit(v);name=key.split('/verified_',1)[1]
        old=next(v for k,v in discovery.items() if k.endswith('/'+name))
        if {k:x for k,x in v['prescribed_case'].items() if k!='capture_undo'} != old['prescribed_case']:raise ValueError('native repeat inputs')
        if result(v)!=result(old) or h['geometry_changed'] is not old['history_changed']:raise ValueError('native repeat outcome')
        for phase in ['before','after']:
            maximum=max(maximum,difference(normalized(v[phase]),normalized(old[phase])))
        if h['recorded'] and not h['geometry_changed']:unchanged_records.append(name)
    if maximum>1e-6 or unchanged_records!=['zero_meeting_parts']:raise ValueError('native repeat or equal-geometry record')
    control_summary,control_captures,extra=controls(proof);paths.update(extra)
    captures.extend(control_captures);captures.extend(final_captures)
    counts.update(geometry_changed=29,discovery_observations=40,history_control_observations=3,unchanged_geometry_records=1,maximum_native_repeat_difference=maximum,retained_failed_captures=control_summary['retained_failed_captures'])
    return rows,captures,counts,paths
