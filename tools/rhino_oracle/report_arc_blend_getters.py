"""Reconstruct nested option/default behavior and the preserved-preview case."""
import argparse
import hashlib
import json
import re
import subprocess
from .arc_blend_getter_evidence import collect
from .check_arc_blend_getters import ROOT,PROOF,commands
from .report_arc_blend import records
BASELINE='ded6c738aef28dc52c7e2562e121846d941d6618'
def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,research,captures,picks,evidence=collect(proof);cases={k:dict(id=k)for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log='getter-'+stage+'-'+mode;marker='ARC_BLEND_GETTER_'+stage.upper()
            rows=records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_arc_sample_error')
            for r in rows:
                if r['warning']is not ('Unable to achieve'in workflows[r['id']]['history']):raise ValueError('getter warning changed')
                cases[r['id']][mode+'_'+stage+'_error']=r['maximum_coefficient_and_sample_error']
                cases[r['id']][mode+'_'+stage+'_native_sample_error']=r['maximum_native_arc_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'src/app/arc_blend/radius.rs','crates/viboceros-command/src/arc_blend.rs','src/app/tests/arc_blend_radius_getters.rs'}.issubset(paths):raise ValueError('implementation lacks nested getters')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='RadiusDifference inside the ratio getter, Enter defaults, remembered targets within the active command, numeric pointer suppression and preserved previews when active targets change.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        matched_native_workflows_per_build=9,retained_partition_research_observations=14,native_physical_picks=picks,archive_round_trips_per_build=18,independent_native_point_tangent_stations=891,
        maximum_coefficient_and_native_sample_difference=max(c[k]for c in cases.values()for k in c if k.endswith('_error')),
        cases=[cases[k]for k in sorted(cases)],research_observations=sorted(research),captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Only accepted input values configure the live ratio/target state. An option may defer an active target while retaining the preview; serialized final arguments then preserve the accepted ratio. Native output is expected data only.',
        execution='Private Xvfb captures and windowless application/egui tests; no desktop window.',
        limits=[
            'The two same-circle command partitions repeat exactly. Recovered searches through the public SDK, dimensional/normalized join formulas, bounds, tie and midpoint variants do not reproduce those command partitions; all fourteen investigations remain explicit research rather than matched representation claims.',
            'Native qualification covers the active command and explicit targets. Remembered values between commands, unit-suffixed radius entry, SubCrv and exact hit-box/depth ranking remain unsupported or unqualified.',
            'Nested Escape returns one local prompt level and stale references reset the editor in windowless tests. Native Escape timing and every pointer trajectory are unqualified.',
            'Historical general-search and creation records retain their original scopes. General continuous distance, comparative speed and same-circle ratio-handle station parity remain unqualified.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/arc-blend-getters-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
