"""Capture immutable native grip extraction, including original source files."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .extract_points_cases import FAMILIES, request

HERE = Path(__file__).resolve().parent
SOURCES = ['client.py','component_picking.py','hole_picking.py','group_picking.py','managed_component_input.py','extract_points_getter.py', 'extract_points_cases.py', 'extract_points_probe.py', 'generate_extract_points.py',
           'extract_points_geometry.py', 'extract_points_component_cases.py', 'extract_points_component_input.py',
           'grip_navigation_probe.py', 'grip_transform_probe.py', 'join_probe.py',
           'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--family', choices=list(FAMILIES), default='grips')
    family = parser.parse_args().family
    name = 'extract_points_'+family
    output = HERE/'observations'/(name+'.json')
    if output.exists():
        raise ValueError('native observations are immutable')
    query = HERE/'fixtures'/(name+'.json')
    query.write_text(json.dumps(request(family), indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name:digest(HERE/name) for name in SOURCES}
    for source_name, sha in sources.items():
        archive = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE/source_name).read_bytes(), mtime=0))
    print('Capturing {} ExtractPt scenes'.format(len(request(family)['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleExtractPtGrips20261024'+family.capitalize()).run_rhino(request(family), timeout=600 if family=='component_picked' else 240)
    if sources != {name:digest(HERE/name) for name in SOURCES}:
        raise ValueError('native ExtractPt producer changed during capture')
    if any('error' in row for row in data['results']) or len(data['results']) != len(request(family)['operations']):
        raise ValueError('incomplete native ExtractPt grip capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False)+'\n')
    folder = HERE/'fixtures'/name
    folder.mkdir(exist_ok=True)
    for row in data['results']:
        (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded native ExtractPt grip scenes', flush=True)


if __name__ == '__main__':
    main()
