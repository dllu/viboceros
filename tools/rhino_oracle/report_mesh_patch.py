"""Complete MeshPatch SDK, command and getter replays at the fixed model gate."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
 'crates/viboceros-geometry/src/mesh/patch.rs','crates/viboceros-geometry/src/mesh/patch/tests.rs',
 'crates/viboceros-geometry/src/mesh/patch/constraints.rs',
 'crates/viboceros-geometry/src/curve/polyline.rs','crates/viboceros-geometry/src/curve/polyline/angle.rs',
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/lib.rs','crates/viboceros-geometry/src/error.rs',
 'crates/viboceros-geometry/src/point_projection.rs','crates/viboceros-geometry/src/point_projection/fit.rs','crates/viboceros-geometry/src/point_projection/fit/normalized.rs',
 'crates/viboceros-geometry/src/frame.rs','crates/viboceros-geometry/src/bezier.rs','crates/viboceros-geometry/src/mesh/projection.rs','crates/viboceros-geometry/src/nurbs_surface/closest_point.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/object_selection.rs','crates/viboceros-command/src/mesh_patch.rs',
 'crates/viboceros-command/src/uv_reference.rs',
 'crates/viboceros-command/tests/mesh_patch_native.rs','crates/viboceros-command/tests/support/mesh_patch.rs',
 'src/app.rs','src/app/object_selection.rs','src/app/transform_sources.rs','src/app/object_selection/mesh_patch.rs','src/app/object_selection/mesh_patch/tests.rs',
 'tools/rhino_oracle/mesh_patch_probe.py','tools/rhino_oracle/generate_mesh_patch.py','tools/rhino_oracle/report_mesh_patch.py','tools/rhino_oracle/test_mesh_patch.py',
 'tools/rhino_oracle/fixtures/mesh_patch.json','tools/rhino_oracle/observations/mesh_patch.json',
 'crates/viboceros-command/src/mesh_patch/tests.rs','tools/rhino_oracle/client.py','tools/rhino_oracle/rhino_worker.py','tools/rhino_oracle/structural_sdk.py',
]
def summarize(log):
    q=json.loads((HERE/'fixtures/mesh_patch.json').read_text());native=json.loads((HERE/'observations/mesh_patch.json').read_text())
    ids=[o['id']for o in q['operations']]
    failures=[r['id']for r in native['results']if r['value']['input'].get('expect_failure')]
    groups={}
    for key,prefix in [('geometry','MESH_PATCH_GEOMETRY'),('command','MESH_PATCH_COMMAND'),('getter','MESH_PATCH_GETTER')]:
        rows=[json.loads(s)for s in re.findall(prefix+r' (\{[^\n]+\})',log)]
        if [r['id']for r in rows]!=ids:raise ValueError('incomplete or duplicate '+prefix+' replay')
        for row in rows:
            if row['qualified']is not True:raise ValueError('unqualified mesh patch hidden')
            if bool(row.get('native_failure'))!=(row['id']in failures):raise ValueError('native failure must remain explicit')
            error=row.get('maximum_bidirectional_sample_error',0 if row['id']in failures else math.inf)
            if not math.isfinite(error)or not 0<=error<1e-6:raise ValueError('invalid mesh patch comparison')
        groups[key]=dict(recipes=len(rows),qualified_recipes=sum(r['qualified']for r in rows),cases=rows)
    return dict(model_epsilon=1e-6,full_native_parity=False,performance_qualified=False,native_engine_version=native['engine_version'],native_construction_failures=failures,
        maximum_sample_difference=max(r.get('maximum_bidirectional_sample_error',0)for group in groups.values()for r in group['cases']),
        geometry_cases=groups['geometry'],command_cases=groups['command'],getter_cases=groups['getter'],
        guarantees=['The 21 captured public SDK/command inputs retain geometry, source snapshots, names, layers, selection and grouped Undo/Redo through command and staged getter replays.','Automatic guide-region trimming, explicit holes, temporary fitted perimeters, point clouds, circle approximation and a curved starting surface are exercised.','A source/guide duplicate that leaves no trimmed faces fails before admission, with native selection release and no Undo record.','Projection conflicts, crossings of explicit outer boundaries, open hole boundaries and resource exhaustion are rejected before geometry edits.','A staged prompt retains immutable source snapshots and a history token; cancellation preserves Redo and stale inputs cannot create a patch.'],
        limits=['The geometry comparisons use barycentric triangle samples; they do not prove universal mesh topology or triangle ordering.','General curved guide sampling, periodic/folded starting charts and starting B-rep trim boundaries need broader qualification.','The projection chart must resolve distinct spatial samples. Guides recover in input order without generating crossing vertices; the separate crossing report retains unresolved native trimming differences.','Angular approximation controls direction variation and has bounded work/subdivision/output budgets; no universal positional certificate is claimed.','Input/perimeter records are capped at 100000, constraint walk/recovery work at 4000000. Performance is not yet compared against native Rhino.'],
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('logs',type=Path,nargs='+');a=p.parse_args()
    (ROOT/'docs/mesh-patch-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in a.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
