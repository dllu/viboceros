"""Immutable authored surface pairs and owned headless SDK observations."""
import gzip
import hashlib
import json
from pathlib import Path
from .merge_surface_cases import PILOT, HOLDOUTS, FINAL, CASES, request
from .fillet_getter_evidence import finite

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'
PROOF = ROOT / 'docs/merge-surfaces-proof'
FAMILIES = {'merge_surfaces_pilot': PILOT, 'merge_surfaces_holdouts': HOLDOUTS,
            'merge_surfaces_final': FINAL}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def frozen(provenance, paths):
    for digest in provenance['sources'].values():
        path = HERE / ('observations/source_snapshots/' + digest + '.gz')
        if sha(gzip.decompress(path.read_bytes())) != digest:
            raise ValueError('frozen source changed')
        paths.add(path)


def collect(proof=PROOF):
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES):
        raise ValueError('capture inventory')
    paths = {proof / 'captures.json', proof / 'capture-attempts.json', HERE / 'run_headless.sh'}
    rows, captures = {}, []
    for receipt, (family, recipes) in zip(receipts, FAMILIES.items()):
        fixture = HERE / ('fixtures/' + family + '.json')
        archive = HERE / ('observations/' + family + '.json.gz')
        raw = gzip.decompress(archive.read_bytes())
        data = json.loads(raw)
        if (json.loads(fixture.read_bytes()) != request(list(recipes))
                or data['engine'] != 'rhino' or data['iterations'] != 1
                or [r['id'] for r in data['results']] != list(recipes)):
            raise ValueError('native input inventory')
        if (data['provenance']['request_sha256'] != sha(fixture.read_bytes())
                or receipt['request_sha256'] != sha(fixture.read_bytes())
                or receipt['observation_sha256'] != sha(raw)):
            raise ValueError('capture hashes')
        if (receipt['exit_status'] != 0 or receipt['archive'] != 'observations/' + family + '.json.gz'
                or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m',
                                         'tools.rhino_oracle.generate_' + family]):
            raise ValueError('headless capture receipt')
        log = proof / receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines() != [
                'Capturing ' + str(len(recipes)) + ' cases: ' + family, 'Recorded ' + family]:
            raise ValueError('capture log')
        required = {name + '.py' for name in ['generate_' + family, 'merge_surfaces_sdk',
                    'merge_surface_cases', 'rhino_worker', 'structural_sdk', 'client',
                    'capture_family', 'brep_join_probe', 'cap_probe']}
        if set(data['provenance']['sources']) != required:
            raise ValueError('frozen source inventory')
        paths.update([fixture, archive, log])
        frozen(data['provenance'], paths)
        captures.append(dict(family=family, observations=len(recipes),
                             engine_version=data['engine_version'], raw_sha256=sha(raw)))
        for row in data['results']:
            key, value = row['id'], row['value']
            success = key not in {'merge_large_gap', 'merge_bowed_boundary'}
            if (key in rows or value['prescribed_case'] != CASES[key] or not finite(value)
                    or value['source_unchanged'] is not True or value['success'] is not success
                    or (value['output'] is not None) is not success):
                raise ValueError('native recipe or result')
            if len(value['inputs']) != 2 or (success and len(value['output']['surfaces']) != 1):
                raise ValueError('native support inventory')
            rows[key] = value
    attempts = json.loads((proof / 'capture-attempts.json').read_bytes())
    if len(attempts) != 1:
        raise ValueError('failed capture inventory')
    for attempt in attempts:
        path = HERE / attempt['archive']
        raw = gzip.decompress(path.read_bytes())
        value = json.loads(raw)
        if (not path.name.endswith('-' + sha(raw) + '.json.gz')
                or value['error'] != attempt['error']
                or 'Attempted relative import in non-package' not in value['error']
                or value['provenance']['request_sha256'] != sha((HERE / 'fixtures/merge_surfaces_pilot.json').read_bytes())):
            raise ValueError('failed capture receipt')
        frozen(value['provenance'], paths)
        paths.add(path)
    if len(rows) != 30 or sum(v['success'] for v in rows.values()) != 28:
        raise ValueError('native coverage counts')
    return rows, captures, paths
