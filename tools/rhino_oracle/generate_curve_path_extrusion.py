"""Capture immutable profile/path splitting and source/history policies."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .client import OracleClient
from .curve_path_extrusion_cases import request,continuity_request

HERE=Path(__file__).resolve().parent
SOURCES=['generate_curve_path_extrusion.py','curve_path_extrusion_cases.py','curve_path_extrusion_probe.py',
         'surface_apex_tangent_cases.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--continuity',action='store_true')
    args=parser.parse_args()
    family='curve_path_continuity' if args.continuity else 'curve_path_extrusion'
    query=HERE/('fixtures/'+family+'.json');output=HERE/('observations/'+family+'.json')
    if output.exists():raise ValueError('native curve path captures are immutable')
    q=continuity_request() if args.continuity else request()
    query.write_text(json.dumps(q,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name) for name in SOURCES}
    print('Capturing {} curve path extrusions'.format(len(q['operations'])),flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleCurvePath20261024').run_rhino(q,timeout=600)
    if sources!={name:digest(HERE/name) for name in SOURCES}:raise ValueError('native producer changed during capture')
    if len(result['results'])!=len(q['operations']) or any('error' in row for row in result['results']):raise ValueError('incomplete curve path extrusion capture')
    result['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        archive=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():archive.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded curve path extrusion commands',flush=True)


if __name__=='__main__':main()
