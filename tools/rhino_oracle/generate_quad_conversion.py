"""Capture immutable public triangle-to-quad SDK references."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .quad_conversion_cases import request
HERE=Path(__file__).resolve().parent
FAMILY='mesh_quad_conversion_v3'
SOURCES=['mesh_channel_capture.py','generate_quad_conversion.py','quad_conversion_cases.py','quad_conversion_probe.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']

def main():
    output=HERE/'observations'/(FAMILY+'.json')
    if output.exists():raise ValueError('observations are immutable; add a family')
    query=HERE/'fixtures'/(FAMILY+'.json');payload=request()
    query.write_text(json.dumps(payload,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    for p,sha in sources.items():
        target=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/p).read_bytes(),mtime=0))
    print('Capturing public quad conversion',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleQuadConversion20261025V3').run_rhino(payload,timeout=240)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(data['results']))+' references',flush=True)
if __name__=='__main__':main()
