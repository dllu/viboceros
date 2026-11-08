"""Pinned finite support-merge evidence; interactive MergeSrf remains pending."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_merge_surfaces import ROOT, PROOF, commands
from .merge_surfaces_evidence import collect, sha

BASELINE = 'db0fa841f8faee64a94bd2878c7345426b14256b'


def comparisons(proof=PROOF):
    native, captures, paths = collect(proof)
    log = proof / 'kernel-native.log.gz'
    paths.add(log)
    text = gzip.decompress(log.read_bytes()).decode()
    rows = {}
    for line in text.splitlines():
        if not line.startswith('MERGE_SURFACE_SUPPORT '): continue
        row = json.loads(line.split(' ', 1)[1])
        key = row['id']
        if key in rows or key not in native:
            raise ValueError('incomplete or duplicate comparison')
        if row['success'] is not native[key]['success']:
            raise ValueError('native result gate')
        if row['success']:
            error = row['maximum_geometry_error']
            if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-9:
                raise ValueError('finite geometry gate')
        rows[key] = row
    if set(rows) != set(native): raise ValueError('incomplete or duplicate comparison')
    for name in ['oriented_supports_match_public_sdk_without_borrowed_input_mutation',
                 'transposed_supports_merge_along_v_with_the_same_surface_image',
                 'unequal_degrees_keep_the_first_nonunit_transverse_clock',
                 'invalid_options_and_directions_leave_sources_usable']:
        if text.count('test ' + name + ' ... ok') != 1:
            raise ValueError('missing adapter regression')
    return rows, captures, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None:
        raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if ([(c['name'], c['command']) for c in checks] != list(commands())
            or any(c['status'] for c in checks)):
        raise ValueError('incomplete checks')
    rows, captures, paths = comparisons(proof)
    paths.update(p for p in proof.iterdir() if p.is_file())
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    required = {'crates/viboceros-smlib/src/native/surface_merge.rs',
                'crates/viboceros-smlib/native/surface_merge.h',
                'crates/viboceros-smlib/tests/surface_merge.rs'}
    if not required.issubset(changed): raise ValueError('missing implementation')
    return dict(scope='Oriented untrimmed NURBS support merging before B-rep and command integration.',
        baseline_revision=BASELINE, implementation_revision=revision,
        observations=len(rows), successful=sum(v['success'] for v in rows.values()), declined=2, failed_capture_attempts=1,
        implementation='Certify complete affine boundary correspondence, import owned SMLib operands, join with zero knot-removal tolerance, restore the degree-multiple seam, then spread only its two outer knots for Smooth/Roundness. Retain first-support transverse parameters.',
        rational_weight_policy='Average homogeneous seam controls without normalizing relative operand weight scales, retaining the public SDK behavior measured by four final rational cases.',
        comparison='Authored operands only; native fitted controls are expectations. Compare degrees, counts, knot clocks and a 17 by 17 parameter grid per successful surface within 1e-9 model units. Input immutability, invalid options, V-direction transpose covariance and two native declines are tested.',
        maximum_sampled_geometry_error=max(v.get('maximum_geometry_error', 0.) for v in rows.values()),
        interactive_command_implemented=False, brep_topology_qualified=False,
        continuous_surface_distance_qualified=False, rhino_performance_qualified=False,
        limits=['This is the support primitive for MergeSrf; command getters, automatic edge selection, history and output B-rep topology remain pending.',
                'Boundary admission requires a supported complete affine parameter correspondence. Trimmed inputs, nonlinear edge-speed matching and broader closed/periodic cases remain unqualified.',
                'Smoothing deliberately changes the surface. Finite native surface comparisons do not establish a continuous surface-distance or derivative-parity bound.'],
        captures=captures, cases=[rows[k] for k in sorted(rows)],
        checks=[dict(name=c['name'], status=c['status']) for c in checks],
        implementation_git_sha256={revision + ':' + p: sha(git('cat-file', 'blob', revision + ':' + p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in sorted(paths)})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision', required=True)
    args = parser.parse_args()
    (ROOT / 'docs/merge-surfaces-validation.json').write_text(
        json.dumps(build(PROOF, args.implementation_revision), indent=2, allow_nan=False) + '\n')
