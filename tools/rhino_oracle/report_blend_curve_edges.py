"""Reconstruct bounded surface-edge BlendCrv qualification from frozen evidence."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_edges_evidence import collect,BASELINE,inputs
from .blend_curve_evidence import command_result
from .check_blend_curve_edges import ROOT,PROOF,commands
from .report_arc_blend import records
FIELDS=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']
def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,diagnostics,captures,counts,evidence=collect(proof)
    cases={k:dict(id=k,inputs=[dict(action=i,parent=n,surface_constraint=s)for i,n,s in inputs(v)])for k,v in workflows.items()}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker='edge-'+stage+'-'+mode,'BLEND_CURVE_EDGES_'+stage.upper()
            rows=records(proof,log,marker,workflows,FIELDS[0],archives=True)
            for field in FIELDS[1:3]:records(proof,log,marker,workflows,field)
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for phase in ['before','after']for o in v[phase])
                if row['native_stations']!=(33 if accepted else 0)or row['accepted']is not accepted or row['derivative_stations']!=count:raise ValueError('edge replay classification changed')
                absolute,domain,ratio=[row[f]for f in FIELDS[3:]]
                if any(type(x)not in[int,float]or not math.isfinite(x)or x<0 for x in[absolute,domain,ratio]):raise ValueError('invalid edge replay diagnostic')
                spans=[abs(o['curve']['domain'][1]-o['curve']['domain'][0])for phase in ['before','after','undo','redo']for o in v[phase]if 'curve'in o]
                bound=max(2e-6,1e-8*max(spans,default=0.))
                if ratio>1 or domain>bound or ratio+1e-12<domain/bound:raise ValueError('edge fractional domain gate failed')
                for field in FIELDS:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/blend_curve/surface.rs','crates/viboceros-command/src/surface_edge_reference.rs','crates/viboceros-command/src/blend_curve/commit.rs','src/app/ui.rs','src/app/tests/blend_curve_edges.rs','tools/rhino_oracle/blend_curve_edge_pick.py'}
    if not required.issubset(paths):raise ValueError('missing surface-edge implementation')
    maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='Explicit Edges mode creates cross-edge surface constraints. Curves mode preserves boundary-curve blends. G0-G2 regular surface constraints, planar G3/G4, angle options, contact/shape/Shift edits and mixed-source trimming/joining are checked.',
        baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
        native_observations=48,matched_document_workflows_per_build=41,accepted_document_workflows=40,native_command_results=dict(Nothing=40,Cancel=1),native_success_boolean=False,
        **counts,source_identity_checked=True,archive_round_trips_per_build=82,
        maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),maximum_normalized_derivative_absolute_difference=maximum(FIELDS[3]),maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),
        retained_failed_capture_attempts=1,retained_validation_attempts=[dict(log='spatial-pick-replay-attempt.log.gz',reason='Perspective captures contain multiple frustum hits; replay now requires a unique nearest screen hit and retains every candidate.'),dict(log='stale-parent-test-attempt.log.gz',reason='The stale-parent test initially replaced a parent with equal geometry, which the document correctly treats as a no-op. It now translates the parent before checking invalidation.'),dict(log='duplicate-module-lint-attempt.log.gz',reason='Clippy rejected loading a shared measurement helper twice; the editor tests now import its existing canonical module.')],
        cases=[cases[k]for k in sorted(cases)],diagnostics=[dict(id=k,reason=v)for k,v in diagnostics.items()],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Shared immutable edge/face references supply oriented boundary tangents and regular surface normals. Normal curvature uses the shape operator in the selected tangent direction. Angle changes and contact edits retain physical handle lengths. Surface parents never contribute join pieces; mixed ordinary curves retain existing trim/join ownership. Native builds use usd-brep endpoint interpolation.',
        input_evidence='Prescribed surfaces are constructed independently of output geometry. Passive pre-input PickContext frustum tests retain all candidate edges, parameters, screen distances, face UVs, normals and trim orientations. Replays use the unique nearest screen hit. Native output coefficients, samples and derivative observations are expectations only.',
        execution='Private Xvfb captures and windowless egui checks; no desktop windows.',
        limits=['Curved-face G3/G4 constraints are explicitly rejected and their complete Native observations retained as diagnostics. Planar G3/G4 have zero higher surface derivatives.',
                'AtAngle_1/2 numeric options are implemented; Alt dragging remains unimplemented. Shared edges requiring face choice and seam edges with multiple trims remain unsupported. Trimmed boundaries, closed edges, singular surface frames and arbitrary overlap picking are unqualified.',
                'The 41 matching workflows cover rectangular plane, cubic/rational/nonorthogonal surface parents, Top and Perspective views, mixed ordinary curves, point endpoints, contact and shape edits, Shift symmetry, cancellation, source identities, undo/redo and 3DM round trips.',
                'The parameter-domain gate retains the previously measured Native default desired fractional length precision. This increment has no additional tight-length measurements, continuous-distance proof or comparative performance qualification.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-edges-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
