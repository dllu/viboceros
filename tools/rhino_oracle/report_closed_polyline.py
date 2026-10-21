"""Complete closed-polyline SDK and selection qualification with visible gaps."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
GROUPS=[('',21),('_variants',10)]
STAGES=['geometry','command','preselected','postselected']
SOURCES=[
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/mesh/closed_polyline.rs',
    'crates/viboceros-geometry/src/mesh/closed_polyline/tests.rs',
    'crates/viboceros-geometry/src/mesh/mass_triangles.rs',
    'crates/viboceros-geometry/src/point.rs',
    'crates/viboceros-geometry/src/polyline.rs',
    'crates/viboceros-geometry/src/curve/polyline.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/mesh_polyline.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/tests/mesh_polyline_native.rs',
    'crates/viboceros-command/tests/support/closed_polyline.rs',
    'src/app.rs','src/app/object_selection.rs',
    'src/app/object_selection/mesh_polyline_tests.rs',
    'tools/rhino_oracle/mesh_polyline_probe.py',
    'tools/rhino_oracle/generate_mesh_polyline.py',
    'tools/rhino_oracle/report_closed_polyline.py',
    'tools/rhino_oracle/test_mesh_polyline.py',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/rhino_worker.py',
    'tools/rhino_oracle/structural_sdk.py',
] + [f'tools/rhino_oracle/{directory}/mesh_polyline{suffix}.json'
     for suffix,_ in GROUPS for directory in ['fixtures','observations']]


def native_rows():
    rows=[]
    for suffix,count in GROUPS:
        q=json.loads((HERE/f'fixtures/mesh_polyline{suffix}.json').read_text())
        r=json.loads((HERE/f'observations/mesh_polyline{suffix}.json').read_text())
        ids=[op['id']for op in q['operations']]
        if len(ids)!=count or [row['id']for row in r['results']]!=ids:
            raise ValueError('incomplete native closed-polyline capture')
        rows.extend(r['results'])
    if len({row['id']for row in rows})!=len(rows):raise ValueError('duplicate capture id')
    return rows


def comparison(row,target):
    if type(row.get('qualified'))is not bool:raise ValueError('missing geometry qualification')
    if row.get('local_empty')is True:
        qualified=target is None
        if row.get('native_empty')is not qualified:raise ValueError('hidden native mesh')
    elif target is None:
        qualified=False
        if row.get('native_empty')is not True:raise ValueError('hidden native empty mesh')
    else:
        error=row.get('maximum_bidirectional_sample_error')
        if type(error)not in (float,int)or not math.isfinite(error)or error<0:
            raise ValueError('invalid mesh distance')
        for key in ['actual_vertices','actual_faces','native_vertices','native_faces']:
            if type(row.get(key))is not int or row[key]<0:raise ValueError('invalid mesh counts')
        if row['native_vertices']!=len(target['vertices'])or row['native_faces']!=len(target['faces']):
            raise ValueError('native mesh counts changed')
        counts=row['actual_vertices']==row['native_vertices']and row['actual_faces']==row['native_faces']
        if row.get('counts_match')is not counts:raise ValueError('hidden topology gap')
        qualified=counts and error<=1e-6
    if row['qualified']is not qualified:raise ValueError('hidden closed-polyline geometry gap')
    return qualified


def summarize(log):
    native=native_rows();ids=[r['id']for r in native];values={r['id']:r['value']for r in native}
    stages={}
    for stage in STAGES:
        rows=[json.loads(s)for s in re.findall(r'CLOSED_POLYLINE_'+stage.upper()+r' (\{[^\n]+\})',log)]
        if [r['id']for r in rows]!=ids:raise ValueError('incomplete or duplicate '+stage+' replay')
        for row in rows:
            value=values[row['id']]
            cancelled=any(e['name']=='MeshPolyline'and e['result']=='Cancel'for e in value['events'])
            if stage=='geometry':
                if len(row['comparisons'])!=len(value['sdk']):raise ValueError('missing SDK comparison')
                qualified=all([comparison(c,t)for c,t in zip(row['comparisons'],value['sdk'])])
            elif stage=='command'and cancelled:
                if row.get('native_cancelled')is not True:raise ValueError('native cancellation hidden')
                qualified=False
            else:
                if stage!='command'and row.get('native_cancelled')is not cancelled:
                    raise ValueError('native cancellation changed')
                check=row['check'];targets=[o['mesh']for o in value['after_script']if o['role']=='output']
                if any(type(check.get(key))is not int or check[key]<0 for key in ['native_meshes','actual_meshes']):
                    raise ValueError('invalid output counts')
                if check['native_meshes']!=len(targets):raise ValueError('native output count changed')
                qualified=check['actual_meshes']==len(targets)
                if len(check['meshes'])!=min(check['actual_meshes'],len(targets)):
                    raise ValueError('missing command mesh comparison')
                matches=[comparison(c,t)for c,t in zip(check['meshes'],targets)]
                aggregate=(max((c.get('maximum_bidirectional_sample_error',0)for c in check['meshes']),default=0)
                           if check['actual_meshes']==len(targets)else None)
                reported=check.get('maximum_bidirectional_sample_error')
                if aggregate is None:
                    if reported is not None:raise ValueError('unmatched outputs have no comparable surface distance')
                elif type(reported)not in (float,int)or not math.isfinite(reported)or reported!=aggregate:
                    raise ValueError('invalid aggregate distance')
                qualified=qualified and all(matches)
                if check['sources_and_attributes_preserved']is not True:
                    raise ValueError('source metadata comparison missing')
                if check['qualified']is not qualified or row['grouped_history']is not qualified:
                    raise ValueError('hidden workflow gap')
            if row['qualified']is not qualified:raise ValueError('hidden workflow qualification gap')
        stages[stage]=dict(recipes=len(rows),qualified_recipes=sum(r['qualified']for r in rows),
                           unqualified_recipes=[r['id']for r in rows if not r['qualified']],cases=rows)
    return dict(model_epsilon=1e-6,full_native_parity=False,performance_qualified=False,recipes=len(ids),stages=stages,
        guarantees=[
            'Each input is triangulated independently; nested polylines are separate meshes rather than holes.',
            'Successful command and pre/postselection replays compare geometry, raw vertex/face counts, source storage, names, colors, layers, selection and grouped Undo/Redo.',
            'Closed degree-one NURBS polylines are recognized. Open boundaries remain ineligible independently of document tolerance.',
            'A single-boundary constrained triangulation classifies the exterior by flood fill. An 8192-edge boundary exercises complete edge retention without a per-triangle boundary scan.',
            'Construction is capped at 100000 combined source vertices; invalid explicit IDs/options fail before document edits. Empty geometry and cancelled selection preserve Redo.',
        ],limits=[
            'The native self-crossing capture returns one partial triangle. Local construction returns no mesh; this geometry and workflow gap is retained explicitly.',
            'Three native recipes cancel: a collinear boundary and two open near/far closures. They have SDK and UI document-state replays, but direct CLI cancellation parity is unqualified.',
            'Bidirectional surface samples and counts do not prove universal triangulation, native vertex/face ordering or normal orientation.',
            'General projection-ambiguous spatial polygons and native comparative speed require further qualification.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    (ROOT/'docs/closed-polyline-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
