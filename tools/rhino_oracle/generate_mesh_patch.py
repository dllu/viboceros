"""Capture public MeshPatch results with immutable input/source provenance."""
import gzip,hashlib,json
from pathlib import Path
from .mesh_patch_probe import request
HERE=Path(__file__).resolve().parent

def main():
    from .client import OracleClient
    q=request();path=HERE/'fixtures/mesh_patch.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_mesh_patch.py','mesh_patch_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing MeshPatch',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleMeshPatch20261020').run_rhino(q,timeout=300)
    Path('/tmp/viboceros-mesh-patch-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    assert sources=={name:digest(HERE/name)for name in names}
    for row in r['results']:
        if 'error'in row:raise ValueError(row)
        value=row['value'];outputs=[o for o in value['after_script']if o['role']=='output']
        if value['input'].get('expect_failure'):
            if outputs or value['sdk']is not None:raise ValueError('unexpected native success: '+row['id'])
        elif not outputs:raise ValueError('no command output: '+row['id']+' '+value['history'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/mesh_patch.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded MeshPatch',flush=True)
if __name__=='__main__':main()
