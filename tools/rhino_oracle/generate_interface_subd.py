"""Capture immutable native interface state, including SubD appearance."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .references.interface_subd import request

HERE=Path(__file__).resolve().parent
SOURCES=['generate_interface_subd.py','references/interface_subd.py','client.py',
    'rhino_worker.py','interface_commands_probe.py']

def main():
    query=HERE/'fixtures/interface_subd.json'
    output=HERE/'observations/interface_subd.json'
    if query.exists() or output.exists():raise ValueError('native interface captures are immutable')
    query.write_text(json.dumps(request(),indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name) for name in SOURCES}
    for name,sha in sources.items():
        archive=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():archive.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing {} interface cases'.format(len(request()['operations'])),flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleInterfaceSubD20261024').run_rhino(request(),timeout=300)
    if sources!={name:digest(HERE/name) for name in SOURCES}:raise ValueError('native producer changed during capture')
    if (any('error' in r for r in result['results']) or len(result['results'])!=len(request()['operations'])):
        raise ValueError('native interface capture is incomplete')
    result['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded native SubD interface states',flush=True)

if __name__=='__main__':main()
