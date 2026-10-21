"""Capture fixed public PlanarMesh inputs in a private headless session."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .planar_mesh_probe import request,post_request
HERE=Path(__file__).resolve().parent


def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--postselect',action='store_true');args=parser.parse_args()
    name='planar_mesh_postselect'if args.postselect else'planar_mesh'
    path=HERE/('fixtures/'+name+'.json');q=post_request()if args.postselect else request()
    path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_planar_mesh.py','planar_mesh_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing PlanarMesh',flush=True)
    r=OracleClient(settings_scheme='VibocerosOraclePlanarMesh'+('Postselect'if args.postselect else'')+'20261021').run_rhino(q,timeout=300)
    Path('/tmp/viboceros-planar-mesh-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    assert sources=={name:digest(HERE/name)for name in names}
    for row in r['results']:
        if 'error'in row:raise ValueError(row)
        if len([e for e in row['value']['events']if e['name']=='PlanarMesh'])!=1:raise ValueError('incomplete '+row['id'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded PlanarMesh',flush=True)


if __name__=='__main__':main()
