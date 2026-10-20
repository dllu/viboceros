"""Capture bounded public Surface.ShortPath SDK geometry in an owned process."""
import gzip,hashlib,json
from pathlib import Path
from .surface_short_path_probe import request,validate_request
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    q=request();validate_request(q);path=HERE/'fixtures/surface_short_path.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_short_path.py','surface_short_path_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names};print('Capturing',len(q['operations']),'short paths',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleShortPath20261020').run_rhino(q,timeout=300)
    assert sources=={name:digest(HERE/name)for name in names}
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/surface_short_path.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded ShortPath geometry',flush=True)
if __name__=='__main__':main()
