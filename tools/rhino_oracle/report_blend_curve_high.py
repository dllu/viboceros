"""Reconstruct the bounded higher-order BlendCrv qualification."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_high_evidence import collect, BASELINE
from .blend_curve_evidence import output
from .blend_curve_length_audit import measure
from .check_blend_curve_high import ROOT, PROOF, commands
from .report_arc_blend import records

def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None:
        raise ValueError('full implementation revision required')
    git = lambda *a: subprocess.check_output(['git', *a], cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}')
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks = json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed checks')
    workflows,captures,picks,stations,derivatives,zero_ends,evidence = collect(proof)
    cases = {k:dict(id=k) for k in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log,marker = 'blend-'+stage+'-'+mode,'BLEND_CURVE_'+stage.upper()+'_HIGH'
            rows = records(proof,log,marker,workflows,'maximum_coefficient_and_sample_error',archives=True)
            records(proof,log,marker,workflows,'maximum_native_sample_error')
            records(proof,log,marker,workflows,'maximum_normalized_derivative_error')
            for row in rows:
                count = sum(len(o.get('derivative_samples',[])) for p in ['before','after'] for o in workflows[row['id']][p])
                if row['native_stations'] != 33 or row['accepted'] is not True or row['derivative_stations'] != count:
                    raise ValueError('higher-order replay classification changed')
                domain = row['maximum_parameter_domain_error']
                absolute = row['maximum_absolute_derivative_error']
                if (type(domain) not in [int,float] or not math.isfinite(domain) or not 0 <= domain <= 2e-6
                        or type(absolute) not in [int,float] or not math.isfinite(absolute) or absolute < 0):
                    raise ValueError('parameter or derivative diagnostic gate failed')
                for field in ['maximum_coefficient_and_sample_error','maximum_native_sample_error',
                              'maximum_normalized_derivative_error','maximum_absolute_derivative_error',
                              'maximum_parameter_domain_error']:
                    cases[row['id']][mode+'_'+stage+'_'+field] = row[field]
    paths = git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required = {'crates/viboceros-geometry/src/nurbs/evaluate/higher.rs',
                'crates/viboceros-geometry/src/endpoint_interpolation.rs',
                'crates/viboceros-smlib/native/knot_refinement.cpp',
                'crates/viboceros-command/src/blend_curve/guide.rs','src/app/tests/blend_curve.rs'}
    if not required.issubset(paths): raise ValueError('missing higher-order implementation')
    sha = lambda b: hashlib.sha256(b).hexdigest()
    def maximum(field): return max(c[mode+'_'+stage+'_'+field] for c in cases.values() for mode in ['portable','native'] for stage in ['command','ui'])
    return dict(scope='BlendCrv G3/G4 Native default construction, mixed endpoint orders, polynomial and rational sources, shape edits, sliding, Flip, reversal, Trim/Join and history.',
        baseline_revision=BASELINE,implementation_revision=revision,model_epsilon=1e-6,
        normalized_derivative_relative_epsilon=1e-6,parameter_domain_epsilon=2e-6,
        full_native_parity=False,new_native_observations=19,promoted_historical_observations=6,
        matched_document_workflows_per_build=25,native_command_results=dict(Nothing=25),native_success_boolean=False,
        native_physical_picks=picks,independent_native_point_tangent_stations=stations,
        independent_native_derivative_stations=derivatives,observed_zero_fourth_constrained_endpoints=zero_ends,
        archive_round_trips_per_build=50,mixed_endpoint_order_pairs_per_backend=25,
        maximum_geometry_difference=max(maximum('maximum_coefficient_and_sample_error'),maximum('maximum_native_sample_error')),
        maximum_normalized_derivative_relative_difference=maximum('maximum_normalized_derivative_error'),
        maximum_normalized_derivative_absolute_difference=maximum('maximum_absolute_derivative_error'),
        maximum_parameter_domain_difference=maximum('maximum_parameter_domain_error'),
        independent_native_domain_length_audit=measure(output(workflows['blend_curve_holdouts/g3_arc'])['curve']),
        cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
        construction='One-sided homogeneous derivatives through fourth order; normal source third jets, speed-scaled Hermite targets and exact minimum-degree endpoint interpolation. Native builds use usd-brep. Fourth targets are zero for Native default compatibility; the generic endpoint-jet API also supports nonzero fourth targets. Native output controls never seed construction.',
        execution='Private Xvfb native captures; windowless application and egui tests.',
        limits=[
            'Observed G4 is a Native construction policy, not a claim of mathematical G4 continuity for arbitrary sources: constrained blend endpoints have zero fourth derivative even on quartic, quintic and rational sources with nonzero fourth derivatives.',
            'Coefficient and point/tangent comparisons use corresponding normalized charts at model epsilon 1e-6. Raw domain error has a separate 2e-6 gate: Native uses approximate length domains, while the implementation integrates length more accurately. Derivatives through order four are scaled by domain width powers; component errors divide by max(1,abs(actual),abs(expected)) before the 1e-6 gate. Absolute errors are retained.',
            'Six historical G3/G4 observations are promoted only in this new qualification. The prior G0-G2 record retains its nine original diagnostics and failed dispatch attempt. Its 25 workflows and legacy Blend constructions are regression checked.',
            'Native Nothing with success=false is retained alongside complete input, output and history evidence. Continuous distance, comparative speed, extreme higher-order scales, closed sources, SubCrv, point-only endpoints, symmetry/Alt edits, curvature display and history-based re-editing remain unqualified.'
        ],implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in paths},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(evidence)})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--implementation-revision',required=True)
    a=p.parse_args()
    (ROOT/'docs/blend-curve-high-validation.json').write_text(json.dumps(build(PROOF,a.implementation_revision),indent=2,allow_nan=False)+'\n')
if __name__=='__main__': main()
