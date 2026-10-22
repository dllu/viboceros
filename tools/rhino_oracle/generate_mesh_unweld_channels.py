"""Capture immutable fixed normal-preservation and normal-update observations."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_unweld_channels_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_unweld_channels.py','mesh_unweld_channels_cases.py',
         'mesh_unweld_channels_probe.py','mesh_channel_capture.py','client.py','rhino_worker.py','structural_sdk.py']


def main():
    query=HERE/'fixtures/mesh_unweld_channels.json'
    output=HERE/'observations/mesh_unweld_channels.json'
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query.write_text(json.dumps(request(),indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p) for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(request()['operations']))+' mesh unwelding recipes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleMeshUnweldChannels20261022').run_rhino(request(),timeout=300)
    if sources!={p:sha(HERE/p) for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(result['results']))+' recipes; errors='+str([r['id'] for r in result['results'] if 'error'in r]),flush=True)


if __name__=='__main__':main()
