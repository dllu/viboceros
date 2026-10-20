"""Capture fixed public surface offset command getters and undo/redo."""
import gzip,hashlib,json
from pathlib import Path
from .surface_offset_command_probe import request,validate_request
from .structural_sdk import PROBES
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    q=request();validate_request(q);path=HERE/'fixtures/surface_offset_command.json';path.write_text(json.dumps(q,indent=2)+'\n')
    module,deps=PROBES['surface_offset_command'];names=['generate_surface_offset_command.py','client.py','rhino_worker.py','structural_sdk.py']+[n+'.py'for n in(module,)+deps]
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={n:digest(HERE/n)for n in names};print('Capturing',len(q['operations']),'surface offset command scenes',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleSurfaceOffsetCommand20261020').run_rhino(q,timeout=180)
    assert sources=={n:digest(HERE/n)for n in names}
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/surface_offset_command.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded surface offset command scenes',flush=True)
if __name__=='__main__':main()
