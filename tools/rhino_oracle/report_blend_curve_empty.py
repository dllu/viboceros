"""Pin endpoint trim/join comparisons to the tested implementation commit."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from .blend_curve_empty_evidence import collect, BASELINE
from .blend_curve_evidence import command_result
from .check_blend_curve_empty import ROOT, PROOF, commands
from .report_arc_blend import records

FIELDS = ['maximum_coefficient_and_sample_error','maximum_native_sample_error',
          'maximum_normalized_derivative_error','maximum_absolute_derivative_error',
          'maximum_parameter_domain_error','maximum_parameter_domain_gate_ratio']

def comparisons(proof):
    workflows, captures, counts, paths = collect(proof)
    cases = {key:dict(id=key) for key in workflows}
    for mode in ['portable','native']:
        for stage in ['command','ui']:
            log = 'empty-'+stage+'-'+mode
            marker = 'BLEND_CURVE_EMPTY_'+stage.upper()
            rows = records(proof,log,marker,workflows,FIELDS[0],archives=True)
            for field in FIELDS[1:3]:
                records(proof,log,marker,workflows,field)
            for row in rows:
                value = workflows[row['id']]
                accepted = command_result(value) == 'Nothing'
                derivatives = sum(len(obj.get('derivative_samples',[])) for phase in ['before','after'] for obj in value[phase])
                if row['accepted'] is not accepted or row['native_stations'] != (33 if accepted else 0) or row['derivative_stations'] != derivatives:
                    raise ValueError('endpoint replay classification changed')
                if any(type(row[f]) not in [int,float] or not math.isfinite(row[f]) or row[f] < 0 for f in FIELDS[3:]) or row[FIELDS[-1]] > 1:
                    raise ValueError('endpoint domain or derivative diagnostic changed')
                spans = [abs(obj['curve']['domain'][1]-obj['curve']['domain'][0]) for phase in ['before','after','undo','redo'] for obj in value[phase] if 'curve' in obj]
                bound = max(2e-6,1e-8*max(spans,default=0.))
                if row[FIELDS[-2]] > bound or row[FIELDS[-1]]+1e-12 < row[FIELDS[-2]]/bound:
                    raise ValueError('endpoint domain or derivative diagnostic changed')
                for field in FIELDS:
                    cases[row['id']][mode+'_'+stage+'_'+field] = row[field]
    return cases,captures,counts,paths

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '') is None:
        raise ValueError('full endpoint implementation revision required')
    git = lambda *args: subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}')
    git('merge-base','--is-ancestor',BASELINE,revision)
    checks = json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks] != list(commands()) or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed endpoint checks')
    cases,captures,counts,paths = comparisons(proof)
    changed = git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    required = {'crates/viboceros-command/src/blend_curve/commit.rs',
                'crates/viboceros-command/tests/blend_curve_empty.rs','src/app/tests/blend_curve_empty.rs'}
    if not required.issubset(changed):
        raise ValueError('missing endpoint trim/join implementation')
    sha = lambda data: hashlib.sha256(data).hexdigest()
    maximum = lambda field: max(c[mode+'_'+stage+'_'+field] for c in cases.values() for mode in ['portable','native'] for stage in ['command','ui'])
    return dict(scope='BlendCrv retains a source when trimming would leave an exactly empty interval; endpoint joins orient sources independently of the flipped blend tangent.',
                baseline_revision=BASELINE,implementation_revision=revision,full_native_parity=False,
                native_observations=len(cases),**counts,model_epsilon=1e-6,normalized_derivative_relative_epsilon=1e-6,
                parameter_domain_absolute_floor=2e-6,parameter_domain_relative_tolerance=1e-8,
                maximum_geometry_difference=max(maximum(FIELDS[0]),maximum(FIELDS[1])),
                maximum_normalized_derivative_relative_difference=maximum(FIELDS[2]),
                maximum_absolute_derivative_difference=maximum(FIELDS[3]),
                maximum_parameter_domain_difference=maximum(FIELDS[4]),maximum_parameter_domain_gate_ratio=maximum(FIELDS[5]),
                archive_round_trips_per_build=2*len(cases),source_identity_attributes_selection_and_history_checked=True,
                retained_validation_attempts=5,
                tolerance_test_correction='A broader portable library attempt had 1676 passes, one stale Line-admission assertion and three ignored tests. The corrected test exercises document tolerance through Polyline; focused tolerance tests run in both modes. The failed full-run log remains.',
                input_evidence='Prescribed public sources, getter options and measured input clicks construct replays; output geometry supplies expectations only.',
                execution='Native captures on private Xvfb displays; application tests are windowless.',
                limits=['The corpus covers lines, arcs, cubic curves, reversed sources, G0/G2/G4, point endpoints, temporary portions and short nonzero trims. It does not establish arbitrary self-blend or surface-boundary join behavior.',
                        'Earlier G3/G4 mathematical-continuity and approximate-length diagnostics remain. Continuous distance, curvature display, history reediting and comparative performance remain unqualified.'],
                cases=[cases[k] for k in sorted(cases)],captures=captures,checks=[dict(name=c['name'],status=c['status']) for c in checks],
                implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
                evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision',required=True)
    args = parser.parse_args()
    (ROOT/'docs/blend-curve-empty-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')

if __name__ == '__main__':
    main()
