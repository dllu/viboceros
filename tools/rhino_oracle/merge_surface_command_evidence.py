"""Retained owned headless command captures, including incomplete attempts."""
import gzip
import json
from .merge_surfaces_evidence import ROOT, HERE, sha, frozen
from .fillet_getter_evidence import finite
from .merge_surface_command import DISCOVERY, QUALIFIED, GETTERS, HOLDOUTS, request
from .merge_surface_orientation_cases import CASES as ORIENTATION, request as orientation_request

PROOF = ROOT / 'docs/merge-srf-command-proof'
FAMILIES = dict(merge_surface_orientation=ORIENTATION,
    merge_surface_command_discovery=DISCOVERY, merge_surface_command_qualified=QUALIFIED,
    merge_surface_command_getters=GETTERS, merge_surface_command_holdouts=HOLDOUTS)
INCOMPLETE = {'merge_d_post_sharp', 'merge_d_post_smooth', 'merge_q_getters'}


def collect(proof=PROOF):
    paths = {proof / 'captures.json', proof / 'capture-attempts.json', HERE / 'run_headless.sh'}
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES): raise ValueError('capture inventory')
    rows, orientation, captures = {}, {}, []
    for receipt, (family, recipes) in zip(receipts, FAMILIES.items()):
        fixture = HERE / ('fixtures/' + family + '.json')
        archive = HERE / ('observations/' + family + '.json.gz')
        raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
        query = orientation_request() if family == 'merge_surface_orientation' else request(list(recipes))
        if (json.loads(fixture.read_bytes()) != query or data['engine'] != 'rhino'
                or data['iterations'] != 1 or [r['id'] for r in data['results']] != list(recipes)
                or receipt['exit_status'] != 0 or receipt['archive'] != 'observations/' + family + '.json.gz'
                or receipt['request_sha256'] != sha(fixture.read_bytes())
                or data['provenance']['request_sha256'] != sha(fixture.read_bytes())
                or receipt['observation_sha256'] != sha(raw)
                or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m',
                                         'tools.rhino_oracle.generate_' + family]):
            raise ValueError('headless capture receipt or native inventory')
        log = proof / receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines() != [
                'Capturing ' + str(len(recipes)) + ' cases: ' + family, 'Recorded ' + family]:
            raise ValueError('capture log')
        required = {'generate_' + family + '.py', 'rhino_worker.py', 'structural_sdk.py', 'client.py', 'capture_family.py'}
        required.update({'merge_surface_orientation_sdk.py', 'merge_surface_orientation_cases.py'}
            if family == 'merge_surface_orientation' else {'merge_surface_command_probe.py',
                'merge_surface_command.py', 'line_scene_probe.py', 'conic_getter_input.py',
                'merge_surface_point_picker.py', 'chamfer_point_picker.py'})
        if not required.issubset(data['provenance']['sources']): raise ValueError('producer inventory')
        frozen(data['provenance'], paths); paths.update([fixture, archive, log])
        captures.append(dict(family=family, observations=len(recipes), engine_version=data['engine_version'],
                             raw_sha256=sha(raw)))
        for row in data['results']:
            key, v = row['id'], row['value']
            if v['prescribed_case'] != recipes[key] or not finite(v): raise ValueError('authored recipe')
            if family == 'merge_surface_orientation':
                if v['success'] is not True or v['source_unchanged'] is not True: raise ValueError('orientation result')
                orientation[key] = v; continue
            traces = v['input_trace']
            complete = len(traces) == 1 and traces[0]['completed'] == traces[0]['expected'] == len(recipes[key]['actions']) and traces[0]['errors'] == []
            if complete == (key in INCOMPLETE): raise ValueError('owned input completion classification')
            if family.endswith('discovery') or key in INCOMPLETE: continue
            events = [e for e in v['events'] if e['name'] == 'MergeSrf']
            result = 'Cancel' if 'Cancel' in recipes[key]['actions'] else 'Failure' if key in {'merge_h_gap_failure','merge_h_bowed_failure'} else 'Success'
            if (key in rows or len(events) != 1 or events[0]['result'] != result
                    or v['success'] is not (result == 'Success')
                    or v['history_changed'] is not (result == 'Success')
                    or v['returned_idle'] is not True): raise ValueError('command result')
            rows[key] = v
    attempts = json.loads((proof / 'capture-attempts.json').read_bytes())
    if len(attempts) != 4: raise ValueError('failed attempt inventory')
    for attempt in attempts:
        path = HERE / attempt['archive']; raw = gzip.decompress(path.read_bytes()); data = json.loads(raw)
        if (not path.name.endswith('-' + sha(raw) + '.json.gz') or data['error'] != attempt['error']
                or attempt['exit_status'] != 1 or attempt['response_retained'] != (data.get('response') is not None)
                or attempt['observation_sha256'] != sha(raw)
                or attempt['request_sha256'] != data['provenance']['request_sha256']
                or attempt['request_sha256'] != sha((HERE / 'fixtures/merge_surface_command_pilot.json').read_bytes())
                or attempt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m',
                                          'tools.rhino_oracle.generate_merge_surface_command_pilot']):
            raise ValueError('failed attempt receipt')
        frozen(data['provenance'], paths)
        paths.update([path, proof / attempt['log']])
    if len(rows) != 20 or sum(v['success'] for v in rows.values()) != 13 or len(orientation) != 10:
        raise ValueError('coverage counts')
    return rows, orientation, captures, paths
