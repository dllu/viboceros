"""Retain bounded public SDK deformation timings and geometry readback."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE = Path(__file__).resolve().parent


def request():
    ops = [dict(op='morph_fit_benchmark', id='axis_'+axis+'_'+shape, shape=shape, axis=axis)
           for axis in ('Reverse', 'Spatial', 'Skew') for shape in ('Curve', 'Surface', 'Box')]
    q = dict(protocol_version=1, iterations=5, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=ops)
    validate_operations(q)
    return q


def main():
    q = request()
    p = HERE/'fixtures/morph_fit_benchmark.json'
    p.write_text(json.dumps(q, indent=2)+'\n')
    r = OracleClient(settings_scheme='VibocerosOracleMorphFitBenchmark20261020').run_rhino(q, timeout=900.)
    assert len(r['results']) == 9 and all('error' not in row for row in r['results'])
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance'] = dict(request_sha256=digest(p), sources={n: digest(HERE/n) for n in
        ('generate_morph_fit_benchmark.py', 'morph_fit_benchmark_probe.py', 'twist_command_probe.py', 'rhino_worker.py', 'structural_sdk.py')})
    (HERE/'observations/morph_fit_benchmark.json').write_text(json.dumps(r, indent=2, allow_nan=False)+'\n')
    print('Recorded nine native fitting benchmarks', flush=True)


if __name__ == '__main__':
    main()
