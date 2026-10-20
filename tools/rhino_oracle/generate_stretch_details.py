"""Measure native approximation error and segment identities."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE = Path(__file__).resolve().parent


def request():
    ops = [dict(op='stretch_details', id='segments_'+shape, mode='grips', shape=shape, axis='X') for shape in ('circle', 'arc')]
    ops += [dict(op='stretch_details', id='distance_'+shape+'_'+axis, mode='fit', shape=shape, axis=axis)
            for shape in ('Curve', 'Surface', 'Box') for axis in ('Z', 'Spatial', 'Skew')]
    q = dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=ops)
    validate_operations(q)
    return q


def main():
    q = request()
    p = HERE/'fixtures/stretch_details.json'
    p.write_text(json.dumps(q, indent=2)+'\n')
    r = OracleClient(settings_scheme='VibocerosOracleStretchDetails20261020').run_rhino(q, timeout=900.)
    assert len(r['results']) == 11 and all('error' not in row for row in r['results'])
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance'] = dict(request_sha256=digest(p), sources={n: digest(HERE/n) for n in
        ('generate_stretch_details.py', 'stretch_details_probe.py', 'twist_command_probe.py', 'grip_transform_probe.py', 'rhino_worker.py', 'structural_sdk.py')})
    (HERE/'observations/stretch_details.json').write_text(json.dumps(r, indent=2, allow_nan=False)+'\n')
    print('Recorded Stretch details', len(r['results']), flush=True)


if __name__ == '__main__':
    main()
