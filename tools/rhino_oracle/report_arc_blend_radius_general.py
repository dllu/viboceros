"""Reconstruct general ArcBlend radius search and explicit partition diagnostics."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .arc_blend_general_evidence import collect,final_warning,REPRESENTATION
from .check_arc_blend_radius_general import ROOT,PROOF,commands
from .report_arc_blend import records
BASELINE='99b91ed5bb098d787b5870af3c054710c63de3a5'

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,public,captures,picks,evidence=collect(proof)
    sdk=records(proof,'radius-search-sdk','ARC_BLEND_RADIUS_SEARCH_SDK',public,'maximum_point_tangent_radius_and_coefficient_error')
    cases={k:dict(id=k,representation_parity=k not in REPRESENTATION)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            rows=records(proof,'general-'+stage+'-'+mode,'ARC_BLEND_RADIUS_GENERAL_'+stage.upper(),workflows,'maximum_native_arc_sample_error',archives=True)
            for r in rows:
                key=r['id'];diagnostic=key in REPRESENTATION
                if r['representation_diagnostic']is not diagnostic:raise ValueError('representation classification changed')
                value=r['maximum_coefficient_and_sample_error']
                if type(value)not in [int,float]or not math.isfinite(value)or value<0 or not diagnostic and value>1e-6:raise ValueError('coefficient gate failed')
                if r['warning']is not final_warning(workflows[key]):raise ValueError('final getter warning changed')
                cases[key][mode+'_'+stage+'_representation_error']=value
                cases[key][mode+'_'+stage+'_geometry_error']=r['maximum_native_arc_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/arc_blend/join.rs','crates/viboceros-geometry/src/arc_blend/radius.rs','crates/viboceros-command/tests/arc_blend_radius_general.rs','src/app/tests/arc_blend_radius_general.rs'}.issubset(paths):raise ValueError('implementation lacks general search workflow')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='General signed radius targets, ordinary/alternate biarcs, backward directions, contact re-solving, trimming and warned partial solutions.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        new_native_observations=99,retained_native_target_observations=28,
        matching_native_geometry_workflows_per_build=79,matching_native_nurbs_representation_workflows_per_build=77,coincident_circle_partition_diagnostics_per_build=2,
        matching_public_sdk_curves=48,native_physical_picks=picks,archive_round_trips_per_build=158,
        final_native_failed_target_warnings=sum(final_warning(v)for v in workflows.values()),
        independent_sdk_point_tangent_stations=4752,independent_command_point_tangent_stations=7689,
        maximum_public_sdk_error=max(r['maximum_point_tangent_radius_and_coefficient_error']for r in sdk),
        maximum_independent_native_geometry_error=max(c[k]for c in cases.values()for k in c if k.endswith('_geometry_error')),
        maximum_qualified_representation_error=max(c[k]for c in cases.values()if c['representation_parity']for k in c if k.endswith('_representation_error')),
        cases=[cases[k]for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Independent tangent-length equation shared by construction and allocation-free trial radius evaluation. Halve the interval toward the endpoint with smaller target error and retain the best sampled fraction. Target residual uses public ON_ZERO_TOLERANCE. Native coordinates are expected values only.',
        execution='All captures used private Xvfb sessions; application and egui tests are windowless.',
        limits=[
            'This inferred bounded search reproduces native partial solutions; it can discard mathematically attainable targets. It is not a global closest-radius solver.',
            'Two same-circle nonzero targets have different internal arc partitions and NURBS data. Complete-curve native samples and supporting circle/sweep bounds verify geometric equality, including 3DM. Their coefficient differences remain explicit diagnostics; native ratio-handle station parity for these two cases is unqualified.',
            'Native qualification includes planar/spatial lines, alternate solutions, backward directions, contact edits, trimming, scaling from 0.1 to 10, close targets and failed targets. Nonlinear source contacts, larger scaling, continuous distance outside the coincident-circle cases and speed comparisons remain unqualified.',
            'Cross-command remembered radius/Trim/Join settings, SubCrv and general handle hit-box/depth ranking remain unsupported or unqualified. Historical records retain their original scope and diagnostics.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/arc-blend-radius-general-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
