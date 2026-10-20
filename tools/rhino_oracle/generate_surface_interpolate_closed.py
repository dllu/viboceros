"""Capture bounded public closed surface interpolation SDK geometry in an owned process."""
import gzip,hashlib,json
from pathlib import Path
from .surface_interpolate_probe import closed_request as request,validate_request
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    q=request();validate_request(q);path=HERE/'fixtures/surface_interpolate_closed.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_interpolate_closed.py','surface_interpolate_probe.py','surface_short_path_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names};print('Capturing',len(q['operations']),'surface interpolants',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleSurfaceInterpolateClosed20261020').run_rhino(q,timeout=300)
    assert sources=={name:digest(HERE/name)for name in names}
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/surface_interpolate_closed.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded surface interpolation geometry',flush=True)
if __name__=='__main__':main()
