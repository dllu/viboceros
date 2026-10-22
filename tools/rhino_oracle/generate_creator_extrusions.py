"""Record fixed creator geometry without overwriting previous evidence."""
import gzip
import hashlib
import json
from pathlib import Path
from .creator_extrusions_probe import request
from .client import OracleClient
HERE=Path(__file__).resolve().parent


def main():
    destination=HERE/'observations/creator_extrusions.json'
    if destination.exists():raise ValueError('creator observations are immutable')
    query=request();fixture=HERE/'fixtures/creator_extrusions.json';fixture.write_text(json.dumps(query,indent=2)+'\n')
    names=['creator_extrusions_probe.py','generate_creator_extrusions.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing '+str(len(query['operations']))+' creator geometry workflows',flush=True)
    response=OracleClient(settings_scheme='VibocerosOracleCreatorExtrusionsCorrect20261021').run_rhino(query,timeout=1200)
    Path('/tmp/viboceros-tosubd/creator-extrusions-response.json').write_text(json.dumps(response,indent=2)+'\n')
    if sources!={name:digest(HERE/name)for name in names}:raise ValueError('producer changed during capture')
    for row in response['results']:
        if 'error'in row:raise ValueError(row)
    response['provenance']=dict(request_sha256=digest(fixture),sources=sources)
    destination.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded creator extrusion geometry',flush=True)


if __name__=='__main__':main()
