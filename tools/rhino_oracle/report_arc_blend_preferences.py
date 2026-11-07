"""Reconstruct remembered settings qualification from actual captures and checks."""
import argparse
import hashlib
import json
import re
import subprocess
from .arc_blend_preference_evidence import collect
from .check_arc_blend_preferences import ROOT,PROOF,commands
from .report_arc_blend import records
BASELINE='a7b28441c56ec09c520bc22cc607084e6c7cba12'
def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,diagnostics,captures,picks,stations,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log='preferences-'+stage+'-'+mode;marker='ARC_BLEND_PREFERENCES_'+stage.upper()
            rows=records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_arc_sample_error')
            for row in rows:
                value=workflows[row['id']];expected=99 if any('arc_segments'in o for o in value['after'])and value['success'] else 0
                if row['accepted']is not value['success']or row['native_stations']!=expected:raise ValueError('settings replay classification changed')
                cases[row['id']][mode+'_'+stage+'_error']=row['maximum_coefficient_and_sample_error']
                cases[row['id']][mode+'_'+stage+'_native_sample_error']=row['maximum_native_arc_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-command/src/arc_blend/preferences.rs','src/app/arc_blend.rs','src/app/tests/arc_blend_preferences.rs'}.issubset(paths):raise ValueError('implementation lacks remembered settings')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='Registry-owned ArcBlend radius target, Trim and Join across accepted/cancelled invocations; fresh manual ratio and primary solution; typed numeric cancellation and pending redo.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        native_observations=26,matched_native_workflows_per_build=len(workflows),accepted_workflows=sum(v['success']for v in workflows.values()),cancelled_workflows=sum(not v['success']for v in workflows.values()),retained_timeout_diagnostics=sorted(diagnostics),native_physical_picks=picks,archive_round_trips_per_build=2*len(workflows),independent_native_point_tangent_stations=stations,
        maximum_coefficient_and_native_sample_difference=max(c[k]for c in cases.values()for k in c if k.endswith('_error')),cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Accepted input settings are copied into command-instance preferences, outside document history. Hover previews and rejected input never update them. Scripted successful commands use and update the same preferences. Native geometry is expected data only.',
        execution='Private Xvfb captures and windowless application/egui tests; no desktop window.',
        limits=['One numeric cancellation capture needed the driver timeout to cancel the parent getter and remains a diagnostic. A separate complete two-Cancel capture qualifies the typed path; native Escape key timing remains unqualified.',
            'Joined workflows compare NURBS coefficients and 33 evaluated stations against captured native NURBS. Independent native arc/tangent samples cover the fourteen accepted unjoined workflows.',
            'Settings persist for the registry lifetime and are shared across documents, but do not persist across application restarts or through 3DM files. Local tests verify isolation, invalid-input atomicity, script defaults and opening documents; native rejected-script preferences are unqualified.',
            'Historical radius-search and creation qualifications keep their scopes. Unit-suffixed radius entry, nonlinear source contacts, SubCrv, exact hit-box/depth ranking, same-circle partition/handle station parity and comparative speed remain unsupported or unqualified.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/arc-blend-preferences-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
