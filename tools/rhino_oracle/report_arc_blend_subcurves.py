"""Reconstruct the temporary ArcBlend source qualification from saved evidence."""
import argparse
import hashlib
import json
import re
import subprocess
from .arc_blend_subcurve_evidence import collect,BASELINE
from .check_arc_blend_subcurves import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,captures,picks,stations,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log='subcurves-'+stage+'-'+mode;marker='ARC_BLEND_SUBCURVE_'+stage.upper()
            rows=records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_arc_sample_error')
            for row in rows:
                v=workflows[row['id']];output=next((o for o in v['after']if not o['attributes']['name']),{})
                expected=33*(1+len(output['arc_segments']))if 'arc_segments'in output else 0
                if row['native_stations']!=expected or row['accepted']is not v['success']:raise ValueError('subcurve replay classification changed')
                cases[row['id']][mode+'_'+stage+'_error']=row['maximum_coefficient_and_sample_error'];cases[row['id']][mode+'_'+stage+'_native_sample_error']=row['maximum_native_arc_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/arc_blend/guide.rs','src/app/arc_blend/subcurve.rs','src/app/tests/arc_blend_subcurves.rs'}
    if not required.issubset(paths):raise ValueError('implementation lacks temporary sources and interaction')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='ArcBlend hidden SubCrv getter: directed temporary sources, point and length entry, pointer parameter conversion, direction locking on an open line, Trim/Join ownership and shared-parent joining.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        native_observations=len(workflows),matched_workflows_per_build=len(workflows),accepted_workflows=28,cancelled_workflows=1,native_physical_picks_in_matched_workflows=picks,archive_round_trips_per_build=2*len(workflows),independent_native_point_tangent_stations=stations,
        maximum_coefficient_and_native_sample_difference=max(c[k]for c in cases.values()for k in c if k.endswith('_error')),cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Temporary portions share immutable captured parents. Point selections trim the bounded interval and may reverse it; numeric lengths use the shared arc-length and seam solvers. Preview and commit use the same backend. Native output geometry is expected data only.',execution='Private Xvfb captures and windowless application/egui tests; no desktop window.',
        limits=['Qualified sources are lines, planar arcs/circles, cubic NURBS and a polyline, with the recorded Top-view pointer paths. Other curves, B-rep edges, other views and exact viewport hit-box/depth ranking have no native SubCrv command qualification.',
            'Numeric direction locking is qualified on an open line. Closed numeric free confirmation includes a long seam-crossing interval and a short backward interval; other locking/confirmation trajectories remain unqualified.',
            'Independent native samples cover unjoined circular blend outputs. Joined outputs check NURBS coefficients and 33 evaluations of the native representation; general continuous distance, extreme scaling and comparative speed remain unqualified.',
            'Nested typed Cancel and zero lengths are qualified. Nested Escape and stale-reference handling have windowless safety tests; Native Escape parity, multi-portion self-overlap, application-restart preferences and general same-circle internal split/handle behavior remain unqualified. Historical qualifications retain their scopes.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/arc-blend-subcurves-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
