"""Qualify native control-net geometry and exact stored vertex attributes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT/'tools/rhino_oracle'
FAMILIES = ['control_polygon','control_polygon_options','control_polygon_priority']
SOURCES = with_shared_sources([
    'crates/viboceros-opennurbs/build.rs','crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/src/subd.rs','crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h','crates/viboceros-geometry/src/lib.rs',
    'crates/viboceros-geometry/src/subd.rs','crates/viboceros-geometry/src/mesh/ngon.rs',
    'crates/viboceros-command/tests/subd_control_net_native.rs',
    'tools/rhino_oracle/control_polygon_cases.py','tools/rhino_oracle/control_polygon_probe.py',
    'tools/rhino_oracle/generate_control_polygon.py','tools/rhino_oracle/test_control_polygon.py',
    'tools/rhino_oracle/report_subd_control_net.py',
])


def expected():
    rows = {}
    for family in FAMILIES:
        native = json.loads((HERE/'observations'/(family+'.json')).read_text())
        for row in native['results']:
            value = row['value']
            if not value['success'] or 'Unknown' in value['history']:
                continue
            for before in value['before']:
                if 'subd_faces' not in before:
                    continue
                output = next(o for o in value['after'] if o['source'] is None and o['name']==before['name'])
                mesh = output['mesh']
                rows[(family,row['id'],before['name'])] = dict(vertices=len(mesh['vertices']),
                    faces=len(mesh['faces']),ngons=len(mesh['ngons']),normals=len(mesh['normals']),
                    textures=len(mesh.get('texture_coordinates',[])))
    return rows


def summarize(text):
    wanted = expected()
    seen = set()
    rows = [json.loads(s) for s in re.findall(r'^CONTROL_NET_NATIVE (.+)$',text,re.M)]
    for row in rows:
        key = (row['family'],row['id'],row['source'])
        if key not in wanted or key in seen:
            raise ValueError('duplicate or unknown control-net result')
        seen.add(key)
        for field,count in wanted[key].items():
            if type(row.get(field)) is not int or row[field]!=count:
                raise ValueError('different control-net topology or attribute count')
        for field in ['qualified','source_unchanged','vertex_attributes_exact','topology_exact']:
            if row.get(field) is not True:
                raise ValueError('unqualified control-net conversion')
        error = row.get('maximum_point_error')
        if type(error) not in [float,int] or not math.isfinite(error) or not 0<=error<=1e-6:
            raise ValueError('control-net point error exceeds epsilon')
    if seen!=set(wanted):
        raise ValueError('incomplete control-net comparisons')
    paths = list(SOURCES)
    for family in FAMILIES:
        paths += ['tools/rhino_oracle/'+folder+'/'+family+'.json' for folder in ['fixtures','observations']]
        paths += [str(p.relative_to(ROOT)) for p in sorted((HERE/'fixtures'/family).rglob('*.3dm'))]
        native = json.loads((HERE/'observations'/(family+'.json')).read_text())
        paths += ['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz' for d in native['provenance']['sources'].values()]
    return dict(native_engine_version='8.32.26160.13001',compared_workflows=len({k[:2] for k in seen}),
        component_sources=len(rows),model_epsilon=1e-6,
        maximum_point_error=max(r['maximum_point_error'] for r in rows),
        native_geometry_and_texture_priorities_qualified=True,
        subd_command_integration_qualified=False,full_native_parity=False,performance_qualified=False,
        limits=[
            'The Rust geometry API preserves native control-mesh topology, n-gons, crease seams, normals and texture coordinates for this finite corpus.',
            'Texture coordinates and normals are stored on the returned TriangleMesh. Full ExtractControlPolygon command integration, broader edit compatibility and comparative performance remain separate work.',
            'Unknown-input trials are retained as diagnostics and excluded. Source geometry is compared with immutable saved native inputs.',
        ],cases=sorted(rows,key=lambda r:(r['family'],r['id'],r['source'])),
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log',type=Path)
    args = parser.parse_args()
    (ROOT/'docs/subd-control-net-qualification.json').write_text(json.dumps(summarize(args.log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':
    main()
