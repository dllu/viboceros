"""Reconstruct ArcBlend contacts and shared scalar-input qualification."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .arc_blend_source_evidence import collect,target,BASELINE
from .check_arc_blend_sources import ROOT,PROOF,commands
from .report_arc_blend import records

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *a:subprocess.check_output(['git',*a],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command'])for c in checks]!=list(commands())or any(c['status']for c in checks):raise ValueError('incomplete or failed checks')
    workflows,points,diagnostics,captures,picks,stations,evidence=collect(proof);cases={k:dict(id=k)for k in workflows};point_cases={k:dict(id=k)for k in points}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log='sources-'+stage+'-'+mode;marker='ARC_BLEND_SOURCE_'+stage.upper()
            rows=records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_arc_sample_error')
            for row in rows:
                v=workflows[row['id']];output=next(o for o in v['after']if not o['attributes']['name']);expected=33*(1+len(output['arc_segments']))if 'arc_segments'in output else 0
                if row['native_stations']!=expected or row['warning']is not ('Unable to achieve'in v['history']):raise ValueError('source replay classification changed')
                a,b=row['requested_radius_difference'],target(v)
                if (a is None)!=(b is None)or a is not None and (type(a)not in(int,float)or not math.isfinite(a)or abs(a-b)>1e-12*max(1,abs(b))):raise ValueError('requested target changed')
                cases[row['id']][mode+'_'+stage+'_error']=row['maximum_coefficient_and_sample_error'];cases[row['id']][mode+'_'+stage+'_native_sample_error']=row['maximum_native_arc_sample_error']
            for row in records(proof,log,'ARC_BLEND_SOURCE_POINT_'+stage.upper(),points,'maximum_coefficient_and_sample_error',archives=True):point_cases[row['id']][mode+'_'+stage+'_error']=row['maximum_coefficient_and_sample_error']
    paths=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required={'crates/viboceros-command/src/arc_blend/handles.rs','crates/viboceros-drafting/src/point_input/calculator.rs','crates/viboceros-command/src/line.rs','src/app/tests/arc_blend_sources.rs'}
    if not required.issubset(paths):raise ValueError('implementation lacks source and scalar fixes')
    sha=lambda b:hashlib.sha256(b).hexdigest()
    return dict(scope='ArcBlend contacts on arcs, cubic/rational NURBS, polycurves and polylines; rational-pointer parameter conversion; quantity/calculator radius and ratio input; shared whole-expression length suffixes in scalar and world-point getters.',baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,full_native_parity=False,
        native_observations=44,matched_arc_blend_workflows_per_build=35,matching_line_point_workflows_per_build=2,retained_input_diagnostics=sorted(diagnostics),retained_failed_capture_attempts=1,native_physical_picks_in_matched_workflows=picks,archive_round_trips_per_build=74,independent_native_point_tangent_stations=stations,pre_fix_contact_error=0.0357200914943144,
        maximum_coefficient_and_native_sample_difference=max(c[k]for c in [*cases.values(),*point_cases.values()]for k in c if k.endswith('_error')),cases=[cases[k]for k in sorted(cases)],point_cases=[point_cases[k]for k in sorted(point_cases)],captures=captures,checks=[dict(name=c['name'],status=c['status'])for c in checks],
        construction='Source contacts keep native parameters; viewport rational parameters pass through the existing exact correspondence. Whole-expression length suffixes are interpreted once by the shared calculator. Commands and UI share unit-aware ArcBlend numeric options. Native output geometry is expected data only.',execution='Private Xvfb captures and windowless application/egui tests; no desktop window.',
        limits=['Six initial scalar-discovery captures have an extra Enter or a rejected mixed-unit expression, and one nested-unit expression required timeout cancellation; all seven remain explicit diagnostics. A failed IronPython producer import is retained separately and contributes no observations.',
            'Native comparisons cover the prescribed line, arc, rational/cubic NURBS, polycurve and polyline contacts. Free mouse source contacts cover planar arcs and arc polycurves in Top view; other source/view trajectories and exact hit-box/depth ranking remain unqualified.',
            'Independent native samples cover circular outputs, including joined arcs. A joined cubic-source workflow checks NURBS coefficients and 33 evaluations of the native representation; general continuous curve distance remains unqualified.',
            'Native rejection command-dispatch side effects, arbitrary nested unit expressions, B-rep edge contacts, SubCrv, same-circle internal split/handle parity, application-restart preferences and comparative speed remain unsupported or unqualified. Historical qualifications keep their scopes.'],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in paths},evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--implementation-revision',required=True);a=p.parse_args()
    (ROOT/'docs/arc-blend-sources-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
