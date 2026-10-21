"""Capture fixed public closed-polyline meshing observations."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .mesh_polyline_probe import request, variant_request

HERE=Path(__file__).resolve().parent


def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--variants',action='store_true');args=parser.parse_args()
    name='mesh_polyline_variants'if args.variants else'mesh_polyline'
    q=variant_request()if args.variants else request()
    path=HERE/('fixtures/'+name+'.json')
    path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_mesh_polyline.py','mesh_polyline_probe.py','join_probe.py',
           'merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing MeshPolyline',flush=True)
    response=OracleClient(settings_scheme='VibocerosOracleMeshPolyline'+('Variants'if args.variants else'')+'20261021').run_rhino(q,timeout=300)
    Path('/tmp/viboceros-mesh-polyline-last-response.json').write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    assert sources=={name:digest(HERE/name)for name in names}
    for row in response['results']:
        if 'error'in row:raise ValueError(row)
        if len([event for event in row['value']['events']if event['name']=='MeshPolyline'])!=1:
            raise ValueError('incomplete MeshPolyline '+row['id'])
    response['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+name+'.json')).write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded MeshPolyline',flush=True)


if __name__=='__main__':main()
