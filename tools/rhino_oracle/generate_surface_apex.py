"""Capture immutable public SDK surface-to-apex results in licensed Rhino."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .surface_apex_extrusion_probe import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_surface_apex.py', 'surface_apex_extrusion_probe.py',
           'surface_extrusion_probe.py', 'client.py', 'rhino_worker.py']


def main():
    query = HERE/'fixtures/surface_apex_extrusion.json'
    output = HERE/'observations/surface_apex_sdk_rhino_20261024.json'
    if output.exists():
        raise ValueError('native surface apex captures are immutable')
    q = request()
    if json.loads(query.read_text()) != q:
        raise ValueError('surface apex fixtures differ from independent recipes')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE/name) for name in SOURCES}
    print('Capturing {} surface apex recipes'.format(len(q['operations'])), flush=True)
    result = OracleClient(settings_scheme='VibocerosOracleSurfaceApex20261024').run_rhino(q, timeout=300)
    if sources != {name: digest(HERE/name) for name in SOURCES}:
        raise ValueError('native producer changed during capture')
    if any('error' in r for r in result['results']) or len(result['results']) != len(q['operations']):
        raise ValueError('surface apex SDK capture is incomplete: {}'.format(result['results']))
    result['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    for name, sha in sources.items():
        archive = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Recorded native surface apex recipes', flush=True)


if __name__ == '__main__':
    main()
