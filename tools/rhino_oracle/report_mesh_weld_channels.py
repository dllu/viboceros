"""Report every captured Weld comparison without hiding remaining differences."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
ROOT=Path(__file__).resolve().parents[2]
HERE=ROOT/'tools/rhino_oracle'
FAMILIES=['mesh_weld_channels'+suffix for suffix in ['', '_followup', '_prefixes', '_pairs']]
SOURCES=with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/tests/mesh_weld_channels.rs',
    'crates/viboceros-geometry/tests/support/mesh_channels.rs',
    'crates/viboceros-command/tests/mesh_weld_channels.rs',
    'tools/rhino_oracle/fixtures/mesh_weld_channels_gaps.json',
    'tools/rhino_oracle/mesh_weld_channels_cases.py',
    'tools/rhino_oracle/mesh_weld_channels_probe.py',
    'tools/rhino_oracle/generate_mesh_weld_channels.py',
    'tools/rhino_oracle/report_mesh_weld_channels.py',
    'tools/rhino_oracle/test_mesh_weld_channels.py',
])


def summarize(log):
    expected={}
    diagnostics=[]
    paths=list(SOURCES)
    for family in FAMILIES:
        query=HERE/'fixtures'/(family+'.json')
        observation=HERE/'observations'/(family+'.json')
        captured=json.loads(observation.read_text())
        paths += [str(p.relative_to(ROOT)) for p in [query,observation]]
        paths += ['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz' for d in captured['provenance']['sources'].values()]
        for row in captured['results']:
            if row['value']['before']['valid'] is not True:
                diagnostics.append(dict(family=family,id=row['id'],reason='Native source is invalid; not a geometry comparison.'))
            else: expected[(family,row['id'])]=row['value']['after']
    gaps=json.loads((HERE/'fixtures/mesh_weld_channels_gaps.json').read_text())
    rows=[json.loads(s) for s in re.findall(r'^MESH_WELD_CHANNEL_NATIVE (.+)$',log,re.M)]
    seen=set()
    for row in rows:
        if not isinstance(row,dict) or set(row)!={'family','id','differences','qualified','vertices','faces'}:
            raise ValueError('unexpected Weld comparison fields')
        key=(row['family'],row['id'])
        if key not in expected or key in seen: raise ValueError('unexpected or duplicate Weld comparison')
        seen.add(key)
        if row['differences'] != gaps.get(row['id'],[]): raise ValueError('Weld mismatch ledger changed')
        if type(row['qualified']) is not bool or row['qualified'] != (not row['differences']): raise ValueError('invalid Weld qualification')
        for field in ['vertices','faces']:
            if type(row[field]) is not int or row[field] != len(expected[key][field]): raise ValueError('Weld dimensions differ')
    if seen!=set(expected): raise ValueError('missing Weld comparisons')
    return dict(native_engine_version='8.32.26160.13001',compared_cases=len(rows),
                qualified_cases=sum(r['qualified'] for r in rows),diagnostic_cases=diagnostics,
                epsilon=1e-6,full_native_parity=False,performance_qualified=False,
                limits=['Every valid source in four fixed captures is compared; invalid source recipes remain explicit diagnostics.',
                        'Connected-face traversal, survivor ordering and some seam attribute reductions still differ. Each differing case lists its fields in the mismatch ledger.',
                        'RGB colors compare exactly; float positions, UVs and stored normals compare within 1e-6. Face cycles allow rotation.',
                        'Three command regressions check identity, metadata, groups and Undo/Redo. The native captures exercise the SDK, not command selection or UI.',
                        'Alpha blending, larger meshes, mixed opposing normals, broader edit compatibility and comparative performance remain unqualified.'],
                cases=sorted(rows,key=lambda r:(r['family'],r['id'])),
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log',type=Path)
    args=parser.parse_args()
    (ROOT/'docs/mesh-weld-channels-qualification.json').write_text(json.dumps(summarize(args.log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__': main()
