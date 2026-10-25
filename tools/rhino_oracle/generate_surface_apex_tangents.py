"""Capture fixed SplitAtTangents recipes without replacing prior observations."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .surface_apex_tangent_cases import request, angle_request, threshold_request, detail_request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_surface_apex_tangents.py', 'surface_apex_tangent_cases.py',
           'surface_apex_command_probe.py', 'surface_apex_extrusion_probe.py',
           'surface_extrusion_probe.py', 'join_probe.py', 'merge_edges_probe.py',
           'client.py', 'rhino_worker.py', 'structural_sdk.py']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--angles', action='store_true')
    modes.add_argument('--threshold', action='store_true')
    modes.add_argument('--details', action='store_true')
    args = parser.parse_args()
    family = 'surface_apex_tangent_details' if args.details else 'surface_apex_tangent_threshold' if args.threshold else 'surface_apex_tangent_angles' if args.angles else 'surface_apex_tangents'
    query = HERE/('fixtures/'+family+'.json')
    output = HERE/('observations/'+family+'.json')
    if output.exists():
        raise ValueError('native surface apex tangent captures are immutable')
    q = detail_request() if args.details else threshold_request() if args.threshold else angle_request() if args.angles else request()
    query.write_text(json.dumps(q, indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE/name) for name in SOURCES}
    print('Capturing {} surface apex tangent commands'.format(len(q['operations'])), flush=True)
    result = OracleClient(settings_scheme='VibocerosOracleSurfaceApexTangents20261024').run_rhino(q, timeout=360)
    if sources != {name: digest(HERE/name) for name in SOURCES}:
        raise ValueError('native producer changed during capture')
    if any('error' in r for r in result['results']) or len(result['results']) != len(q['operations']):
        raise ValueError('surface apex tangent capture is incomplete: {}'.format(result['results']))
    result['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    for name, sha in sources.items():
        archive = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Recorded native surface apex tangent commands', flush=True)


if __name__ == '__main__':
    main()
