"""Reconstruct point-endpoint and partial-join qualification from immutable evidence."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_points_evidence import collect,BASELINE,initial_pick_point,point_index
from .blend_curve_evidence import command_result
from .check_blend_curve_points import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands()) or any(c['status']for c in checks):
        raise ValueError('incomplete or failed checks')
    workflows,diagnostics,captures,picks,stations,derivatives,joins,cancelled,evidence=collect(proof)
    cases={k:dict(id=k,initial_blend_start='PickPoint' if initial_pick_point(v) else 'CurveEnd',point_endpoint=point_index(v))for k,v in workflows.items()}
    fields=['maximum_coefficient_and_sample_error','maximum_native_sample_error','maximum_normalized_derivative_error','maximum_absolute_derivative_error','maximum_parameter_domain_error']
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker='point-'+stage+'-'+mode,'BLEND_CURVE_POINT_'+stage.upper()
            rows=records(proof,log,marker,workflows,fields[0],archives=True)
            for field in fields[1:3]:records(proof,log,marker,workflows,field)
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing'
                count=sum(len(o.get('derivative_samples',[]))for p in ['before','after']for o in v[p])
                if row['native_stations']!=(33 if accepted else 0) or row['accepted'] is not accepted or row['derivative_stations']!=count:
                    raise ValueError('point replay classification changed')
                domain,absolute=row[fields[4]],row[fields[3]]
                if (type(domain)not in [int,float] or not math.isfinite(domain) or not 0<=domain<=2e-6
                        or type(absolute)not in [int,float] or not math.isfinite(absolute) or absolute<0):
                    raise ValueError('point parameter or derivative diagnostic gate failed')
                for field in fields:cases[row['id']][mode+'_'+stage+'_'+field]=row[field]
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/blend_curve/endpoint.rs','crates/viboceros-command/src/blend_curve/commit.rs',
              'crates/viboceros-geometry/src/curve_blend_pair/jets.rs','src/app/blend_curve/points.rs',
              'src/app/drafting_controls.rs','src/app/snapping.rs','src/app/tests/blend_curve_points.rs'}
    if not required.issubset(paths):raise ValueError('missing point endpoint implementation')
    maximum=lambda field:max(c[mode+'_'+stage+'_'+field]for c in cases.values()for mode in ['portable','native']for stage in ['command','ui'])
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='BlendCrv with one free point and one curve, positional point constraints, G0-G4 curve constraints, editing, partial joins, source ownership, cancellation and command history.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,parameter_domain_epsilon=2e-6,full_native_parity=False,
        native_observations=42,matched_document_workflows_per_build=41,point_endpoint_workflows=38,supplemental_two_curve_join_workflows=3,
        accepted_document_workflows=38,cancelled_workflows=cancelled,native_command_results=dict(Nothing=38,Cancel=3),native_success_boolean=False,
        joined_workflows=joins,source_identity_checked=True,native_physical_picks=picks,independent_native_point_tangent_stations=stations,independent_native_derivative_stations=derivatives,
        archive_round_trips_per_build=82,public_pick_context_receipts=3,exactly_calibrated_earlier_pick_receipts=1,
        maximum_geometry_difference=max(maximum(fields[0]),maximum(fields[1])),maximum_normalized_derivative_relative_difference=maximum(fields[2]),
        maximum_normalized_derivative_absolute_difference=maximum(fields[3]),maximum_parameter_domain_difference=maximum(fields[4]),
        retained_diagnostics=[dict(id=k,reason=diagnostics[k])for k in sorted(diagnostics)],retained_failed_capture_attempts=1,
        cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Free points have no document ownership and force G0; one curve direction uses chord-length speed by default. Shared exact endpoint interpolation uses usd-brep in native builds. Trim-disabled joins include only connected whole sources, leaving disconnected sources untouched; output inherits the first participating source, or current-layer attributes when none participates.',
        input_evidence='Initial public getter prompts restore remembered BlendStart. Public PickContext and ObjRef measurements precede command execution. One earlier spatial pick is calibrated only by exact agreement of prescribed source geometry, view, construction plane, tolerances, source index, pixel and viewing ray. Native output geometry never seeds construction or input matching.',
        execution='Private Xvfb captures and windowless egui tests; no desktop window.',
        limits=[
            'Point is available for at most one endpoint. An attempted second Point sequence ended Failure before all inputs completed; it is retained as a diagnostic, not a matched geometry rejection.',
            'An overlapping-source fixture opened an owned selection menu and was interrupted with exit 130 after screenshot verification. Its original producer snapshots, partial worker log and screenshot are retained; it contributes no native observations.',
            'Native point inputs and geometry are qualified on the recorded Top-view scenes. Windowless tests additionally check 3D point snapping, preview/acceptance, typed input, nested adjustment cancellation, pending redo and stale references. Native snap, numeric/Escape trajectories, exact hit-box/depth ranking and independent reproduction of native pick-frustum parameters remain unqualified.',
            'Some holdout IDs describe an intended shape or copied portion; initial remembered PickPoint changed the selected control or source contact. Actual prompts, settings and outputs are retained. A separate explicit CurveEnd shape case verifies shape editing. Partial joins exclude disconnected sources rather than copying their portions.',
            'Joined source ownership is qualified on lines, including both point orders and mixed/interior two-curve contacts. Point blending also covers analytic arcs and polynomial/rational NURBS. Closed sources, SubCrv, symmetry/Alt edits, curvature display, history re-editing, continuous distance and comparative speed remain unqualified.',
            'G4 follows the previously measured zero-fourth-derivative default and does not establish mathematical G4 continuity on arbitrary sources. Geometry, normalized derivatives and approximate native length domains retain their separate gates from the higher-order qualification.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-points-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
