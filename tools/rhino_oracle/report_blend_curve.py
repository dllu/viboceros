"""Reconstruct the G0-G2 adjustable curve-blend qualification from saved evidence."""
import argparse
import hashlib
import json
import re
import subprocess
from .blend_curve_evidence import collect,BASELINE,command_result,output
from .check_blend_curve import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,diagnostics,captures,picks,stations,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log='blend-'+stage+'-'+mode;marker='BLEND_CURVE_'+stage.upper()
            rows=records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_sample_error')
            for row in rows:
                v=workflows[row['id']];accepted=command_result(v)=='Nothing';expected=33 if accepted else 0
                if row['native_stations']!=expected or row['accepted']is not accepted:raise ValueError('blend replay classification changed')
                cases[row['id']][mode+'_'+stage+'_error']=row['maximum_coefficient_and_sample_error'];cases[row['id']][mode+'_'+stage+'_native_sample_error']=row['maximum_native_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-geometry/src/curve_blend_pair/jets.rs','crates/viboceros-command/src/blend_curve.rs','src/app/blend_curve.rs','src/viewport/blend_curve.rs','src/app/tests/blend_curve.rs'}
    if not required.issubset(paths):raise ValueError('implementation lacks shared interpolation, command or viewport editor')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='BlendCrv G0/G1/G2 endpoint and PickPoint contacts; mixed continuity, shape handles, contact sliding, Flip, Trim/Join ownership, preview and command history.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        native_observations=34,matched_document_workflows_per_build=25,accepted_document_workflows=23,cancelled_workflows=2,native_command_results=dict(Nothing=23,Cancel=2),native_success_boolean=False,native_physical_picks_in_matched_workflows=picks,archive_round_trips_per_build=50,independent_native_point_tangent_stations=stations,retained_diagnostics=[dict(id=k,reason=diagnostics[k])for k in sorted(diagnostics)],retained_failed_capture_attempts=1,
        maximum_coefficient_and_native_sample_difference=max(c[k]for c in cases.values()for k in c if k.endswith('_error')),cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Oriented endpoint jets share the existing exact minimum-degree homogeneous interpolator, using usd-brep in native builds. Endpoint-specific handle defaults are shared with Blend; BlendCrv uses the resulting arc-length domain and retains handle lengths while contacts move. Native output geometry is expected data only.',execution='Private Xvfb captures and windowless application/egui tests; no desktop window.',
        limits=['Native BlendCrv returns Nothing, with success=false, after accepting a valid blend. Matched admission requires its observed output, complete input, finite independent samples and undo/redo; the original result is retained.',
            'Six complete G3/G4 observations remain unsupported higher-derivative diagnostics. Two earlier shape-handle drivers missed a lowercase point prompt; one flip-number attempt timed out after a rejected token. These three input diagnostics are retained separately. A failed dispatch attempt contributes no native observations.',
            'Qualified sources are planar lines, one arc and cubic NURBS in the recorded Top-view workflows. B-rep edges, closed curves, SubCrv, point-only endpoints, G3/G4, arbitrary overlapping self-blends, symmetry/Alt edits and history-based re-editing remain unsupported or unqualified.',
            'Joining is qualified at natural endpoints with and without Trim, and at PickPoint contacts with Trim. Interior Join without Trim is rejected. Degenerate coincident contacts, negative/zero shape lengths, arbitrary control-net edits and source identity after Join remain unqualified.',
            'Unit-aware script and editor lengths, invalid input, stale-reference handling and nested Escape have local tests; corresponding native numeric/Escape trajectories, exact hit-box/depth ranking, continuous curve distance and comparative speed remain unqualified.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/blend-curve-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
