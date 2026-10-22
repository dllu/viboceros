"""Capture immutable ToSubD discovery evidence through an owned headless job."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .tosubd_cases import request

HERE = Path(__file__).resolve().parent


def main():
    from .client import OracleClient
    parser = argparse.ArgumentParser(description=__doc__)
    families = [('discovery','tosubd_discovery','Discovery'),
                ('surface_details','tosubd_surface_details','SurfaceDetails'),
                ('surface_edges','tosubd_surface_edges','SurfaceEdges'),
                ('features','tosubd_features','Features'), ('poles','tosubd_poles','Poles'),
                ('extrusions','tosubd_extrusions','Extrusions'), ('pipes','tosubd_pipes','Pipes')]
    choices = parser.add_mutually_exclusive_group()
    for flag, _, _ in families:
        choices.add_argument('--' + flag.replace('_','-'), action='store_true')
    args = parser.parse_args()
    selected = next(((flag,family,scheme) for flag,family,scheme in families if getattr(args,flag)),
                    (None,'tosubd','Main'))
    flag, family, scheme = selected
    destination = HERE / 'observations' / (family + '.json')
    if destination.exists():
        raise ValueError('observations are immutable; use another family')
    path = HERE / 'fixtures' / (family + '.json')
    query = request(**({flag:True} if flag else {}))
    path.write_text(json.dumps(query, indent=2) + '\n')
    names = ['generate_tosubd.py', 'tosubd_cases.py', 'tosubd_extrusion_cases.py', 'tosubd_probe.py', 'tosubd_info.py', 'subd_box_probe.py',
             'join_probe.py', 'merge_edges_probe.py', 'client.py', 'structural_sdk.py', 'rhino_worker.py']
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in names}
    print('Capturing ' + family, flush=True)
    response = OracleClient(settings_scheme='VibocerosOracleToSubD'+scheme+'20261021').run_rhino(query, timeout=600)
    Path('/tmp/viboceros-tosubd/response.json').write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    if sources != {name: digest(HERE / name) for name in names}:
        raise ValueError('producer changed during capture')
    for row in response['results']:
        if 'error' in row:
            raise ValueError(row)
    response['provenance'] = dict(request_sha256=digest(path), sources=sources)
    destination.write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        snapshot = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not snapshot.exists():
            snapshot.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
