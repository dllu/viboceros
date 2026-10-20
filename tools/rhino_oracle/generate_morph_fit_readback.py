"""Export deformed fixtures, then let Rhino inspect the unchanged 3dm bytes."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .generate_morph_fit_benchmark import request as benchmark
from .structural_sdk import validate_operations
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def request():
    ops = [dict(op='morph_fit_readback', id=o['id'], shape=o['shape'], axis=o['axis'],
                artifact_path=str(HERE/'fixtures/morph_fit_readback'/(o['id']+'.3dm')))
           for o in benchmark()['operations']]
    q = dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=ops)
    validate_operations(q)
    return q


def main():
    q = request()
    subprocess.run(['cargo', 'build', '--release', '-p', 'viboceros-command', '--features', 'native-smlib', '--example', 'profile_morph_fits'], cwd=ROOT, check=True)
    executable = ROOT/'target/release/examples/profile_morph_fits'
    for op in q['operations']:
        subprocess.run([str(executable), op['id'], '1', str(Path(op['artifact_path']).parent)], cwd=ROOT, check=True)
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    artifacts = {Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']}
    p = HERE/'fixtures/morph_fit_readback.json'
    p.write_text(json.dumps(q, indent=2)+'\n')
    r = OracleClient(settings_scheme='VibocerosOracleMorphReadback20261020').run_rhino(q, timeout=900.)
    assert len(r['results']) == 9 and all('error' not in row for row in r['results'])
    assert artifacts == {Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']}
    r['provenance'] = dict(request_sha256=digest(p), sources={n: digest(HERE/n) for n in
        ('generate_morph_fit_readback.py', 'morph_fit_readback_probe.py', 'generate_morph_fit_benchmark.py', 'morph_fit_benchmark_probe.py', 'twist_command_probe.py', 'rhino_worker.py', 'structural_sdk.py')}, fixtures=artifacts)
    (HERE/'observations/morph_fit_readback.json').write_text(json.dumps(r, indent=2, allow_nan=False)+'\n')
    print('Recorded nine deformation 3dm readbacks', flush=True)


if __name__ == '__main__':
    main()
