"""Audit public getter receipts without deriving construction from output curves."""
import base64
import gzip
import hashlib
import json
import math
import struct
from pathlib import Path
from . import fillet_command as recipes

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'
BASELINE = '989b580f12992ca2cc45755557b1ad9c5543ee98'
FAMILIES = dict(fillet_getter_pilot=list(recipes.PILOT), fillet_getter_discovery=list(recipes.DISCOVERY),
                fillet_getter_holdouts=list(recipes.HOLDOUTS), fillet_getter_final=list(recipes.FINAL))
DIAGNOSTICS = {'fillet_getter_pilot/fixed_joined', 'fillet_getter_pilot/dynamic_cancel',
               'fillet_getter_holdouts/dynamic_numeric', 'fillet_getter_holdouts/dynamic_distance'}
REJECTED_JOIN = {'fillet_getter_holdouts/dynamic_distance_cancel': 'No', 'fillet_getter_holdouts/zero_joined': 'Yes'}


def sha(raw): return hashlib.sha256(raw).hexdigest()
def finite(value):
    if isinstance(value, float): return math.isfinite(value)
    if isinstance(value, dict): return all(finite(v) for v in value.values())
    if isinstance(value, list): return all(finite(v) for v in value)
    return True
def normalized(rows): return sorted([{k: v for k, v in row.items() if k != 'selected'} for row in rows], key=lambda r: r['attributes']['name'])
def result(value):
    events = [e for e in value['events'] if e['name'] == 'Fillet']
    if len(events) != 1: raise ValueError('missing native command result')
    return events[0]['result']


def collect(proof):
    paths = {p for p in proof.iterdir() if p.is_file()} | {HERE / 'run_headless.sh'}
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES): raise ValueError('capture inventory changed')
    rows, diagnostics, captures = {}, {}, []
    counts = dict(observations=0, complete=0, diagnostics=0, successful=0, changed=0, cancelled=0, picks=0, samples=0, images=0, rejected_join_options=0)
    for receipt, (family, names) in zip(receipts, FAMILIES.items()):
        fixture = HERE / ('fixtures/' + family + '.json')
        archive = HERE / ('observations/' + family + '.json.gz')
        raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
        if data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != names or json.loads(fixture.read_bytes()) != recipes.request(names): raise ValueError('native input inventory changed')
        if data['provenance']['request_sha256'] != sha(fixture.read_bytes()) or receipt['request_sha256'] != sha(fixture.read_bytes()) or receipt['observation_sha256'] != sha(raw): raise ValueError('native capture hash changed')
        if receipt['exit_status'] != 0 or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + family]: raise ValueError('missing owned capture receipt')
        if gzip.decompress((proof / receipt['log']).read_bytes()).decode().splitlines() != ['Capturing ' + str(len(names)) + ' cases: ' + family, 'Recorded ' + family]: raise ValueError('incomplete native capture')
        paths.update([fixture, archive])
        for digest in data['provenance']['sources'].values():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('frozen producer changed')
            paths.add(path)
        captures.append(dict(family=family, observations=len(names), engine_version=data['engine_version'], raw_sha256=sha(raw)))
        for row in data['results']:
            key, value = family + '/' + row['id'], row['value']; trace = value['input_trace'][0]
            counts['observations'] += 1
            if not finite(value) or value['prescribed_case'] != recipes.CASES[row['id']] or not value['returned_idle']: raise ValueError('invalid native workflow')
            if key in DIAGNOSTICS:
                if 'owned Fillet input timeout' not in trace['errors'] or result(value) != 'Cancel' or value['history_changed']: raise ValueError('diagnostic classification changed')
                diagnostics[key] = 'Radius value prompt or final acceptance Enter omitted by the driver'
                counts['diagnostics'] += 1; continue
            unknown = [line for line in value['history'].splitlines() if line.startswith('Unknown command')]
            expected_unknown = ['Unknown command: _Join=' + REJECTED_JOIN[key]] if key in REJECTED_JOIN else []
            if unknown != expected_unknown: raise ValueError('unexpected rejected native input')
            if key in REJECTED_JOIN:
                index = value['prescribed_case']['actions'].index('Join=' + REJECTED_JOIN[key])
                if 'Trim=No' not in trace['prompts'][index] or 'Join=' in trace['prompts'][index]: raise ValueError('Join rejection did not occur with Trim=No')
                counts['rejected_join_options'] += 1
            if trace['errors'] or trace['completed'] != trace['expected'] or trace['expected'] != len(value['prescribed_case']['actions']): raise ValueError('incomplete native workflow')
            success = result(value) == 'Success'
            if result(value) not in ('Success', 'Cancel') or value['success'] is not success: raise ValueError('native result classification changed')
            for phase in ['before', 'after', 'undo', 'redo']:
                for obj in value[phase]:
                    if not obj['native_valid']: raise ValueError('invalid native object')
                    if 'curve' in obj and len(obj.get('samples', [])) != 33: raise ValueError('missing public curve samples')
            if normalized(value['after']) != normalized(value['redo']) or (value['history_changed'] and normalized(value['before']) != normalized(value['undo'])): raise ValueError('native undo/redo changed')
            if not value['history_changed'] and normalized(value['before']) != normalized(value['after']): raise ValueError('uncommitted native edit')
            if not success and not value['prescribed_case'].get('pending_redo'): raise ValueError('cancellation lacks redo witness')
            for click in trace.get('clicks', []):
                if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray']) != 2 or 'projected_point' not in click: raise ValueError('missing physical pick receipt')
            for shot in trace.get('viewport_snapshots', []):
                image = base64.b64decode(shot.get('png_base64', ''), validate=True)
                if not shot['captured'] or image[:8] != b'\x89PNG\r\n\x1a\n' or len(image) < 32 or struct.unpack('>II', image[16:24]) != (shot['width'], shot['height']): raise ValueError('invalid viewport evidence')
            rows[key] = value; counts['complete'] += 1; counts['successful'] += success; counts['changed'] += value['history_changed']; counts['cancelled'] += not success
            counts['picks'] += len(trace.get('clicks', [])); counts['samples'] += sum(len(o.get('samples', [])) for phase in ['before', 'after'] for o in value[phase]); counts['images'] += len(trace.get('viewport_snapshots', []))
    attempts = json.loads((proof / 'capture-attempts.json').read_bytes())
    if len(attempts) != 2: raise ValueError('failed capture inventory changed')
    for attempt, expected in zip(attempts, ['unsupported oracle operation: fillet_command', 'IndexError at operation curved_contact']):
        fixture = HERE / ('fixtures/' + attempt['family'] + '.json')
        if attempt['status'] != 1 or attempt['native_observations'] != 0 or attempt['request_sha256'] != sha(fixture.read_bytes()) or expected not in gzip.decompress((proof / attempt['log']).read_bytes()).decode(): raise ValueError('failed capture receipt changed')
        paths.add(fixture)
        for digest in attempt['sources'].values():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('failed producer changed')
            paths.add(path)
    intervention = json.loads((proof / 'manual-intervention.json').read_bytes())
    if intervention['case'] != 'curved_contact' or not intervention['window_pid_verified'] or intervention['published_before_intervention']: raise ValueError('manual intervention scope changed')
    original = json.loads((HERE / 'fixtures/fillet_getter_verification.json').read_bytes())
    repeated = json.loads((HERE / 'fixtures/fillet_getter_final.json').read_bytes())
    if original['operations'][:-1] != repeated['operations'][:-1] or recipes.VERIFICATION['curved_contact']['curves'] != recipes.FINAL['curved_contact_separated']['curves']: raise ValueError('failed recipes were not repeated')
    if counts != dict(observations=29, complete=25, diagnostics=4, successful=16, changed=14, cancelled=9, picks=41, samples=3300, images=8, rejected_join_options=2): raise ValueError('qualification coverage changed')
    return rows, diagnostics, captures, counts, paths
