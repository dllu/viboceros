"""Read-only SDK comparison using the existing surface shape fixtures."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE = Path(__file__).resolve().parent

def request():
    rows = json.loads((HERE/'observations/soft_edit_surface_shapes.json').read_text())['results']
    identities = {'flat_small', 'periodic_small', 'periodic_start', 'torus_small', 'torus_start', 'sphere_transpose', 'cylinder_transpose'}
    operations = []
    for row in rows:
        if row['id'] not in identities:
            continue
        value = row['value']
        recipe = value['recipe']
        operations.append(dict(op='soft_edit_surface_sdk', id=row['id'], artifact_path=recipe['artifact_path'], reference=value['reference'], target=value['target'], distance=recipe['distance']))
    result = dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=operations)
    validate_operations(result)
    return result

def main():
    q = request()
    p = HERE/'fixtures/soft_edit_surface_sdk.json'
    p.write_text(json.dumps(q, indent=2)+'\n')
    r = OracleClient(settings_scheme='VibocerosOracleSurfaceSDK20261020').run_rhino(q, timeout=900.)
    assert len(r['results']) == len(q['operations']) and all('error' not in row for row in r['results'])
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance'] = dict(request_sha256=digest(p), sources={n:digest(HERE/n) for n in ['generate_soft_edit_surface_sdk.py', 'soft_edit_surface_sdk_probe.py', 'structural_sdk.py', 'rhino_worker.py']}, fixtures={o['artifact_path']:digest(Path(o['artifact_path'])) for o in q['operations']})
    (HERE/'observations/soft_edit_surface_sdk.json').write_text(json.dumps(r, indent=2, allow_nan=False)+'\n')
    print('Recorded surface SDK cases', len(r['results']), flush=True)

if __name__ == '__main__':
    main()
