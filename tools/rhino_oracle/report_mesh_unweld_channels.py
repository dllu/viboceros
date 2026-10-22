"""Qualify stored-normal policies against a fixed public Mesh.Unweld capture."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
ROOT=Path(__file__).resolve().parents[2]
HERE=ROOT/'tools/rhino_oracle'
SOURCES=with_shared_sources([
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-command/src/lib.rs',
    'crates/viboceros-geometry/tests/mesh_unweld_channels.rs',
    'crates/viboceros-command/tests/mesh_unweld_channels.rs',
    'crates/viboceros-command/tests/support/mesh_seams.rs',
    'tools/rhino_oracle/mesh_unweld_channels_cases.py',
    'tools/rhino_oracle/mesh_unweld_channels_probe.py',
    'tools/rhino_oracle/mesh_channel_capture.py',
    'tools/rhino_oracle/generate_mesh_unweld_channels.py',
    'tools/rhino_oracle/report_mesh_unweld_channels.py',
    'tools/rhino_oracle/test_mesh_unweld_channels.py',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/rhino_worker.py','tools/rhino_oracle/structural_sdk.py',
])


def summarize(geometry_log,command_log):
    observation=HERE/'observations/mesh_unweld_channels.json'
    native=json.loads(observation.read_text())
    expected={r['id']:r['value'] for r in native['results']}
    rows=[json.loads(s) for s in re.findall(r'^MESH_UNWELD_CHANNEL_NATIVE (.+)$',geometry_log,re.M)]
    seen=set()
    for row in rows:
        if set(row)!={'id','differences','qualified','vertices','faces'}:raise ValueError('unexpected comparison fields')
        key=row['id']
        if key not in expected or key in seen:raise ValueError('unexpected or duplicate unweld comparison')
        seen.add(key)
        if row['qualified'] is not True or row['differences']!=[]:raise ValueError('unqualified unweld comparison')
        for field in ['vertices','faces']:
            if type(row[field]) is not int or row[field]!=len(expected[key]['after'][field]):raise ValueError('wrong mesh dimensions')
    if seen!=set(expected):raise ValueError('missing unweld comparisons')
    commands=[json.loads(s) for s in re.findall(r'^MESH_SEAM_COMMAND (.+)$',command_log,re.M)]
    commands=[r for r in commands if r.get('history')=='Unweld']
    seen=set()
    wanted={(key,explicit) for key,v in expected.items() for explicit in ([False,True] if v['input']['modify_normals'] else [True])}
    for row in commands:
        if set(row)!={'id','command','history','qualified'} or row['qualified'] is not True:raise ValueError('unqualified command comparison')
        if row['id'] not in expected:raise ValueError('unknown command input')
        v=expected[row['id']]['input']
        words=row['command'].split()
        explicit=len(words)==3
        key=(row['id'],explicit)
        if key not in wanted or key in seen or len(words) not in [2,3] or words[0]!='Unweld':raise ValueError('unexpected or repeated command')
        if not words[1].startswith('Angle='):raise ValueError('missing command angle')
        angle=float(words[1].split('=')[1])
        if not math.isfinite(angle) or abs(angle-math.degrees(v['angle']))>1e-9:raise ValueError('wrong command angle')
        if explicit and words[2]!='ModifyNormals='+('Yes' if v['modify_normals'] else 'No'):raise ValueError('wrong normal policy')
        seen.add(key)
    if seen!=wanted:raise ValueError('missing command comparisons')
    paths=list(SOURCES)+['tools/rhino_oracle/fixtures/mesh_unweld_channels.json',str(observation.relative_to(ROOT))]
    paths+=['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz' for d in native['provenance']['sources'].values()]
    return dict(native_engine_version=native['engine_version'],compared_cases=len(rows),qualified_cases=len(rows),
                command_cases=len(commands),epsilon=1e-6,full_native_parity=False,performance_qualified=False,
                limits=['The fixed corpus checks both stored-normal policies, angle thresholds, unused vertices, disconnected faces and meshes with no normal channel.',
                        'ModifyNormals updates replacement vertices only; retained vertices keep their stored normals. Preserve mode copies source normals. Neither mode creates an absent channel.',
                        'Command comparisons cover explicit Yes/No and the default Yes, identity, attributes, groups, selection and Undo/Redo.',
                        'Broader non-manifold, curved multi-face normal reductions, edge/vertex command options and comparative performance remain unqualified.'],
                cases=sorted(rows,key=lambda r:r['id']),commands=sorted(commands,key=lambda r:(r['id'],r['command'])),
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('geometry_log',type=Path);parser.add_argument('command_log',type=Path)
    args=parser.parse_args()
    (ROOT/'docs/mesh-unweld-channels-qualification.json').write_text(json.dumps(summarize(args.geometry_log.read_text(),args.command_log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
