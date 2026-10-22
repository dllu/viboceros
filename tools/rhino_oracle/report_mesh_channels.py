"""Retain measured mesh channel comparisons, including the native weld gap."""
import argparse
import hashlib
import json
from pathlib import Path
import re

from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'
SOURCES = with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/src/subd/control_net.rs',
    'crates/viboceros-geometry/tests/mesh_channels.rs',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-io/src/three_dm.rs',
    'crates/viboceros-io/tests/mesh_channels.rs',
    'tools/rhino_oracle/mesh_channels_probe.py',
    'tools/rhino_oracle/generate_mesh_channels.py',
    'tools/rhino_oracle/report_mesh_channels.py',
    'tools/rhino_oracle/test_mesh_channels.py',
    'tools/rhino_oracle/fixtures/mesh_channels.json',
    'tools/rhino_oracle/fixtures/mesh_channel_assets.json',
    'tools/rhino_oracle/observations/mesh_channels.json',
])


def summarize(log):
    native = json.loads((HERE / 'observations/mesh_channels.json').read_text())
    expected = {r['id']: r['value']['after'] for r in native['results']}
    rows = [json.loads(s) for s in re.findall(r'^MESH_CHANNEL_NATIVE (.+)$', log, re.M)]
    seen = set()
    for row in rows:
        key = row['id']
        if key not in expected or key in seen:
            raise ValueError('unexpected or duplicate mesh channel case')
        seen.add(key)
        if (row['vertices'], row['faces']) != (len(expected[key]['vertices']), len(expected[key]['faces'])):
            raise ValueError('mesh dimensions differ')
        if type(row['qualified']) is not bool or row['qualified'] != (not row['differences']):
            raise ValueError('inconsistent mesh channel qualification')
        if not row['qualified'] and key != 'texture_cube_weld':
            raise ValueError('new mesh channel regression')
        if any(d not in ['vertices', 'faces', 'ngons', 'UV', 'normals'] for d in row['differences']):
            raise ValueError('unknown mesh channel difference')
    if seen != set(expected):
        raise ValueError('missing mesh channel comparisons')
    paths = SOURCES + ['tools/rhino_oracle/observations/source_snapshots/' + d + '.gz'
                       for d in native['provenance']['sources'].values()]
    paths += [v['path'] for v in json.loads((HERE / 'fixtures/mesh_channel_assets.json').read_text()).values()]
    return dict(native_engine_version=native['engine_version'], compared_cases=len(rows),
                qualified_cases=sum(r['qualified'] for r in rows), epsilon=1e-6,
                full_native_parity=all(r['qualified'] for r in rows), performance_qualified=False,
                limits=[
                    'Finite raw-vertex channel corpus; face cycles allow rotation and n-gon boundaries compare membership.',
                    'The packed texture cube Weld case retains an explicit vertex-order and seam-averaging gap.',
                    'Broader edit compatibility, texture display and comparative performance remain unqualified.',
                ], cases=sorted(rows, key=lambda r: r['id']),
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    args = parser.parse_args()
    (ROOT/'docs/mesh-channels-qualification.json').write_text(json.dumps(summarize(args.log.read_text()), indent=2) + '\n')


if __name__ == '__main__':
    main()
