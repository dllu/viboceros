"""Capture immutable public mesh attribute operations and exact producer versions."""
import gzip
import hashlib
import json
from pathlib import Path
from .mesh_channels_probe import request

HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_channels.py','mesh_channels_probe.py','fixtures/mesh_channel_assets.json',
         'client.py','rhino_worker.py','structural_sdk.py']


def main():
    from .client import OracleClient
    path=HERE/'fixtures/mesh_channels.json'
    output=HERE/'observations/mesh_channels.json'
    if output.exists():
        raise ValueError('observations are immutable; add a family')
    query=request()
    path.write_text(json.dumps(query,indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p) for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(query['operations']))+' mesh channel operations',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleMeshChannels20261022').run_rhino(query,timeout=600)
    if sources!={p:sha(HERE/p) for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(path),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(result['results']))+' mesh channel operations',flush=True)


if __name__=='__main__':main()
