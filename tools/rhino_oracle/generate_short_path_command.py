"""Capture fixed public ShortPath command getters in an owned process."""
import gzip,hashlib,json
from pathlib import Path
from .short_path_command_probe import request,validate_request
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    q=request();validate_request(q);path=HERE/'fixtures/short_path_command.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_short_path_command.py','short_path_command_probe.py','surface_short_path_probe.py','conic_getter_input.py','join_probe.py','merge_edges_probe.py','snap_environment.py','number_token.py','managed_mouse_callback.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names};print('Capturing',len(q['operations']),'short paths',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleShortPathCommand20261020').run_rhino(q,timeout=300)
    assert sources=={name:digest(HERE/name)for name in names}
    for row in r['results']:
        trace=row['value']['input_trace']
        if trace['errors']or trace['completed']!=trace['expected']:raise ValueError('incomplete ShortPath getter capture: '+row['id'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/short_path_command.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded ShortPath geometry',flush=True)
if __name__=='__main__':main()
