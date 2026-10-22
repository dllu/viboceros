"""Report every captured Weld comparison without hiding remaining differences."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
ROOT=Path(__file__).resolve().parents[2]
HERE=ROOT/'tools/rhino_oracle'
FAMILIES=['mesh_weld_channels'+suffix for suffix in ['', '_followup', '_prefixes', '_pairs', '_stress', '_trace', '_options']]
SOURCES=with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/tests/mesh_weld_channels.rs',
    'crates/viboceros-geometry/tests/support/mesh_channels.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/tests/mesh_weld_channels.rs',
    'crates/viboceros-command/tests/support/mesh_seams.rs',
    'tools/rhino_oracle/fixtures/mesh_weld_channels_gaps.json',
    'tools/rhino_oracle/mesh_weld_channels_cases.py',
    'tools/rhino_oracle/mesh_weld_channels_probe.py',
    'tools/rhino_oracle/mesh_weld_channels_stress_cases.py',
    'tools/rhino_oracle/mesh_weld_channels_trace_cases.py',
    'tools/rhino_oracle/mesh_weld_channels_options_cases.py',
    'tools/rhino_oracle/mesh_channel_capture.py',
    'tools/rhino_oracle/generate_mesh_weld_channels.py',
    'tools/rhino_oracle/report_mesh_weld_channels.py',
    'tools/rhino_oracle/test_mesh_weld_channels.py',
])


def trace_comparisons():
    from .mesh_weld_channels_trace_cases import TWINS
    values={r['id']:r['value'] for suffix in ['', '_stress'] for r in
            json.loads((HERE/'observations'/('mesh_weld_channels'+suffix+'.json')).read_text())['results']}
    captured=json.loads((HERE/'observations/mesh_weld_channels_trace.json').read_text())
    result=[]
    for row in captured['results']:
        v=row['value']; twin=values[TWINS[row['id']]]
        if v['before']['valid'] is not False or v['after']['valid'] is not True:
            raise ValueError('diagnostic validity changed')
        if {k:x for k,x in v['input'].items() if k!='late_invalid_face'} != twin['input']:
            raise ValueError('diagnostic twin recipe differs')
        after=v['after']
        used=sorted({i for face in after['faces'] for i in face})
        remap={original:compact for compact,original in enumerate(used)}
        compact={k:[after[k][i] for i in used] if after[k] else []
                 for k in ['vertices','uv','normals','colors']}
        compact['faces']=[[remap[i] for i in f] for f in after['faces']]
        if any(value != twin['after'][key] for key,value in compact.items()):
            raise ValueError('precompaction diagnostic does not reproduce its valid twin')
        result.append(dict(id=row['id'],twin=TWINS[row['id']],raw_vertices=len(after['vertices']),
                           compact_vertices=len(used),fields_exact=True))
    if {r['id'] for r in result} != set(TWINS): raise ValueError('missing diagnostic twins')
    return result


def radial_comparisons():
    from .mesh_weld_channels_options_cases import RADIAL_TWINS
    values={r['id']:r['value'] for suffix in ['', '_stress'] for r in
            json.loads((HERE/'observations'/('mesh_weld_channels'+suffix+'.json')).read_text())['results']}
    rows=json.loads((HERE/'observations/mesh_weld_channels_options.json').read_text())['results']
    result=[]
    for row in rows:
        if row['id'] not in RADIAL_TWINS:continue
        v=row['value'];twin=values[RADIAL_TWINS[row['id']]]
        if {k:x for k,x in v['input'].items() if k!='sort_radial'}!=twin['input']:raise ValueError('radial twin recipe differs')
        if v['before']!=twin['before'] or v['after']!=twin['after']:raise ValueError('cached radial sorting changed Weld output')
        order=v['radial_sort']
        if [r['vertex']for r in order]!=list(range(len(v['before']['topology_vertices']))):raise ValueError('incomplete radial sort')
        for r in order:
            incident=[i for i,e in enumerate(v['before']['topology_edges']) if r['vertex'] in e['vertices']]
            if r['success'] is not True or sorted(r['before'])!=incident or sorted(r['after'])!=incident:raise ValueError('invalid radial permutation')
        result.append(dict(id=row['id'],twin=RADIAL_TWINS[row['id']],fields_exact=True,
                           changed_radial_vertices=sum(r['before']!=r['after'] for r in order)))
    if {r['id']for r in result}!=set(RADIAL_TWINS):raise ValueError('missing radial twins')
    return result


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
                precompaction_diagnostics=trace_comparisons(),radial_cache_comparisons=radial_comparisons(),
                epsilon=1e-6,full_native_parity=False,performance_qualified=False,
                limits=['Every valid source in seven fixed captures is compared; invalid source recipes remain explicit diagnostics.',
                        'Existing representatives and directed seams close the original packed-cube gap; some face/raw permutations, triangulated cubes and grids still differ. Each difference remains explicit.',
                        'RGBA colors (OpenNURBS alpha is transparency) compare exactly on the captured profiles; float positions, UVs and stored normals compare within 1e-6. Face cycles allow rotation.',
                        'Fifteen command cases check identity, metadata, groups and Undo/Redo. The native captures exercise the SDK, not command selection or UI.',
                        'Four fixed transparency profiles match across three source meshes. Eight cached radial-sort variants reproduce their native twins exactly. Larger meshes, mixed opposing normals, broader edit compatibility and comparative performance remain unqualified.'],
                cases=sorted(rows,key=lambda r:(r['family'],r['id'])),
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log',type=Path)
    args=parser.parse_args()
    (ROOT/'docs/mesh-weld-channels-qualification.json').write_text(json.dumps(summarize(args.log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__': main()
