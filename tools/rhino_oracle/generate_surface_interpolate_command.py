"""Capture fixed public InterpCrvOnSrf command getters in an owned process."""
import gzip,hashlib,json
from pathlib import Path
from .surface_interpolate_command_probe import request,validate_request
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    q=request();validate_request(q);path=HERE/'fixtures/surface_interpolate_command.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_interpolate_command.py','surface_interpolate_command_probe.py','short_path_command_probe.py','surface_short_path_probe.py','conic_getter_input.py','join_probe.py','merge_edges_probe.py','snap_environment.py','number_token.py','managed_mouse_callback.py','catenary_point_picker.py','group_picking.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names};print('Capturing',len(q['operations']),'surface interpolants',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleSurfaceInterpolateCommand20261020').run_rhino(q,timeout=300)
    assert sources=={name:digest(HERE/name)for name in names}
    Path('/tmp/viboceros-surface-interpolate-command-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for row in r['results']:
        trace=row['value']['input_trace']
        if trace['errors']or trace['completed']!=trace['expected']:raise ValueError('incomplete InterpCrvOnSrf getter capture: '+row['id']+' '+json.dumps(trace)+' '+row['value']['history'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/surface_interpolate_command.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded surface interpolation commands',flush=True)
if __name__=='__main__':main()
