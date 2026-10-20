"""Summarize ShortPath replays without relaxing native agreement gates."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
    'crates/viboceros-geometry/src/surface_short_path.rs',
    'crates/viboceros-geometry/src/surface_short_path/tests.rs',
    'crates/viboceros-geometry/tests/surface_short_path_native.rs',
    'crates/viboceros-geometry/src/surface_geodesic.rs',
    'crates/viboceros-geometry/src/morph/curve_fit.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate.rs',
    'crates/viboceros-command/src/short_path.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/tests/short_path_native.rs',
    'crates/viboceros-command/tests/support/short_path.rs',
    'src/app.rs','src/app/short_path.rs','src/app/mesh_face_prompt.rs','src/app/tests/short_path.rs',
    'tools/rhino_oracle/surface_short_path_probe.py','tools/rhino_oracle/short_path_command_probe.py',
    'tools/rhino_oracle/fixtures/surface_short_path.json','tools/rhino_oracle/observations/surface_short_path.json',
    'tools/rhino_oracle/fixtures/short_path_command.json','tools/rhino_oracle/observations/short_path_command.json',
    'tools/rhino_oracle/test_short_path.py','tools/rhino_oracle/report_short_path.py',
    'tools/rhino_oracle/generate_short_path_boundaries.py','tools/rhino_oracle/fixtures/short_path_boundaries.json','tools/rhino_oracle/observations/short_path_boundaries.json',
]
def summarize(log):
    def rows(prefix,ids):
        result=[json.loads(s)for s in re.findall(prefix+r' (\{[^\n]+\})',log)]
        if [r['id']for r in result]!=ids:raise ValueError('incomplete or duplicate '+prefix+' replay')
        for r in result:
            error=r['maximum_bidirectional_sample_error']
            if not math.isfinite(error)or error<0 or r['qualified']!=(error<1e-6):raise ValueError('invalid native qualification row')
        return dict(recipes=len(result),qualified_recipes=sum(r['qualified']for r in result),unqualified_recipes=[r['id']for r in result if not r['qualified']],maximum_sample_difference=max(r['maximum_bidirectional_sample_error']for r in result),cases=result)
    sdk_request=json.loads((HERE/'fixtures/surface_short_path.json').read_text())
    command_request=json.loads((HERE/'fixtures/short_path_command.json').read_text())
    sdk=rows('SURFACE_SHORT_PATH',[op['id']for op in sdk_request['operations']])
    command=rows('SHORT_PATH_COMMAND',[op['id']for op in command_request['operations']if op['case']!='cancel'])
    getter=rows('SHORT_PATH_GETTER',[op['id']for op in command_request['operations']])
    boundary_request=json.loads((HERE/'fixtures/short_path_boundaries.json').read_text())
    boundary_ids=[op['id']for op in boundary_request['operations']]
    boundary_command=rows('SHORT_PATH_BOUNDARY_COMMAND',boundary_ids)
    boundary_getter=rows('SHORT_PATH_BOUNDARY_GETTER',boundary_ids)
    if boundary_command['qualified_recipes']!=8 or boundary_getter['qualified_recipes']!=8:raise ValueError('trim-boundary cases must retain the native gate')
    for row in sdk['cases']:
        if not 0<=row['surface_image_bound']<=5e-7 or not 0<=row['geodesic_queries']<=8192 or not 0<=row['distance']:raise ValueError('invalid complete-image or work bound')
    witness=re.findall(r'SHORT_PATH_CYLINDER_UNROLLED_WITNESS ([^\s]+)',log)
    if len(witness)!=1 or not 5e-4<float(witness[0])<6e-4:raise ValueError('one independent unrolled cylinder witness required')
    return dict(model_epsilon=1e-6,full_native_parity=False,command_registered=True,
        sdk_cases=sdk,command_cases=command,getter_cases=getter,trim_boundary_cases=boundary_command,trim_boundary_getter_cases=boundary_getter,
        bidirectional_samples_per_curve=130,maximum_surface_image_bound=max(r['surface_image_bound']for r in sdk['cases']),
        native_cylinder_same_height_witness=float(witness[0]),
        guarantees=['Requested surface endpoints within model tolerance and immutable parents.','Continuous correspondence bound over every returned curve, within half the model tolerance.','Analytic primitive loci; general paths retain numerical integration and sampled fit estimates.','Native metadata, cancellation, selection, current output layer and Undo/Redo replay.','Face trim holes and concave trim outlines are ignored, matching the captured native command. The viewport constraint also uses the underlying surface.'],
        limits=['General shooting selects the shortest converged candidate; no global minimum certificate.','Natural-boundary detours, general singularities and InfinitePlane point input are not implemented. Face trims are intentionally ignored to match native behavior.','Curved native SDK and command outputs exceed the unchanged native agreement gate in the recorded cases.','Antipodal shortest paths are nonunique; the canonical sphere tie chooses decreasing U.'],
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('replays',type=Path,nargs='+');args=parser.parse_args()
    report=summarize('\n'.join(p.read_text()for p in args.replays))
    (ROOT/'docs/short-path-qualification.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
