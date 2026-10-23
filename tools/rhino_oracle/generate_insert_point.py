"""Capture immutable mesh/SubD InsertPoint scenes and exact before files."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .insert_point_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_insert_point.py','insert_point_cases.py','insert_point_holdout_cases.py','insert_point_ngon_cases.py','insert_point_mesh_shape_cases.py','insert_point_pick_depth_cases.py','insert_point_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py','catenary_point_picker.py','group_picking.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
FAMILIES = {
    'discovery': ('insert_point_discovery', 'Discovery'),
    'live': ('insert_point_live', 'Live'),
    'full': ('insert_point_full', 'Full'),
    'regions': ('insert_point_regions', 'Regions'),
    'holdouts': ('insert_point_holdouts_v2', 'HoldoutsV2'),
    'ngons': ('insert_point_ngons', 'Ngons'),
    'shapes': ('insert_point_mesh_shapes', 'MeshShapes'),
    'depth': ('insert_point_pick_depth', 'PickDepth'),
    'witnesses': ('insert_point_pick_depth_v2', 'PickDepthV2'),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.set_defaults(family='discovery')
    flags = parser.add_mutually_exclusive_group()
    for flag in FAMILIES:
        if flag != 'discovery':
            flags.add_argument('--'+flag, dest='family', action='store_const', const=flag)
    args = parser.parse_args()
    family, suffix = FAMILIES[args.family]
    query = HERE/'fixtures'/(family+'.json')
    output = HERE/'observations'/(family+'.json')
    if output.exists():
        raise ValueError('native observations are immutable; add a family')
    payload = request(**({args.family: True} if args.family != 'discovery' else {}))
    query.write_text(json.dumps(payload, indent=2)+'\n')
    sources = {p: digest(HERE/p) for p in SOURCES}
    for name, sha in sources.items():
        target = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():
            target.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Capturing '+str(len(payload['operations']))+' InsertPoint scenes', flush=True)
    client = OracleClient(settings_scheme='VibocerosOracleInsertPoint20261023'+suffix)
    result = client.run_rhino(payload, timeout=240)
    if sources != {p: digest(HERE/p) for p in SOURCES}:
        raise ValueError('producer changed during capture')
    result['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    folder = HERE/'fixtures'/family
    folder.mkdir(exist_ok=True)
    for row in result['results']:
        if row.get('value',{}).get('source_3dm'):
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(result['results']))+' scenes', flush=True)


if __name__=='__main__':main()
