"""Capture immutable owned ExtrudeSrfToPoint command behavior."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .surface_apex_command_probe import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_surface_apex_command.py', 'surface_apex_command_probe.py',
           'surface_apex_extrusion_probe.py', 'surface_extrusion_probe.py', 'join_probe.py',
           'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py']


def main():
    query = HERE/'fixtures/surface_apex_command.json'
    output = HERE/'observations/surface_apex_command_tolerance.json'
    if output.exists():
        raise ValueError('native surface apex command captures are immutable')
    q = request()
    query.write_text(json.dumps(q, indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE/name) for name in SOURCES}
    print('Capturing {} surface apex commands'.format(len(q['operations'])), flush=True)
    result = OracleClient(settings_scheme='VibocerosOracleSurfaceApexCommand20261024').run_rhino(q, timeout=300)
    if sources != {name: digest(HERE/name) for name in SOURCES}:
        raise ValueError('native producer changed during capture')
    if any('error' in r for r in result['results']) or len(result['results']) != len(q['operations']):
        raise ValueError('surface apex command capture is incomplete: {}'.format(result['results']))
    result['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    for name, sha in sources.items():
        archive = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Recorded native surface apex commands', flush=True)


if __name__ == '__main__':
    main()
