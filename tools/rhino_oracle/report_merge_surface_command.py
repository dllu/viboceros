"""Pinned finite MergeSrf command qualification, with original diagnostics retained."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_merge_surface_command import commands
from .merge_surface_command_evidence import ROOT, PROOF, collect
from .merge_surfaces_evidence import collect as supports, sha

BASELINE = '0a00343c748b4a7bd42c0ac78e125a0ee818c635'


def comparisons(proof=PROOF):
    native, orientation, captures, paths = collect(proof)
    support, _, support_paths = supports()
    paths.update(support_paths)
    geometry = dict(support, **orientation)
    stages = {}
    for mode in ['portable', 'native']:
        registry = proof / ('registry-' + mode + '.log.gz')
        paths.add(registry)
        if gzip.decompress(registry.read_bytes()).decode().count('::reports_sorted_help ... ok') != 1:
            raise ValueError('missing command inventory regression')
        for kind, prefix, log_name, expected in [
                ('geometry', 'MERGE_SURFACE_GEOMETRY ', 'command-', geometry),
                ('api', 'MERGE_SURFACE_API ', 'command-', {k:v for k,v in native.items() if 'Cancel' not in v['prescribed_case']['actions']}),
                ('ui', 'MERGE_SURFACE_UI ', 'ui-', native)]:
            log = proof / (log_name + mode + '.log.gz'); paths.add(log)
            text = gzip.decompress(log.read_bytes()).decode()
            if 'test result: ok.' not in text: raise ValueError('unsuccessful replay')
            rows = {}
            for line in text.splitlines():
                if not line.startswith(prefix): continue
                row = json.loads(line[len(prefix):]); key = row['id']
                if key in rows or key not in expected: raise ValueError('duplicate or unexpected comparison')
                value = expected[key]
                if kind == 'geometry':
                    if row['success'] is not value['success']: raise ValueError('geometry result gate')
                    succeeded = row['success']
                else:
                    event = next(e for e in value['events'] if e['name'] == 'MergeSrf')
                    if row['result'] != event['result'] or row['changed'] is not value['history_changed']:
                        raise ValueError('command result gate')
                    succeeded = True
                if succeeded:
                    error = row['maximum_geometry_error']
                    if (type(error) not in (int,float) or not math.isfinite(error) or not 0 <= error <= 1e-9
                            or type(row['archives']) is not int or row['archives'] != 1):
                        raise ValueError('geometry or archive gate')
                rows[key] = row
            if set(rows) != set(expected): raise ValueError('incomplete comparison inventory')
            stages[kind + '-' + mode] = [rows[k] for k in sorted(rows)]
    return stages, captures, paths


def build(proof, revision):
    if re.fullmatch('[0-9a-f]{40}', revision or '') is None: raise ValueError('full implementation revision required')
    git = lambda *args: subprocess.check_output(['git', *args], cwd=ROOT)
    git('merge-base', '--is-ancestor', BASELINE, revision)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if ([(c['name'],c['command']) for c in checks] != list(commands())
            or any(type(c['status']) is not int or c['status'] != 0 for c in checks)):
        raise ValueError('incomplete qualification checks')
    stages, captures, paths = comparisons(proof)
    paths.update(p for p in proof.iterdir() if p.is_file())
    changed = git('diff', '--name-only', '--diff-filter=ACMR', BASELINE, revision).decode().splitlines()
    required = {'crates/viboceros-command/src/merge_surfaces.rs',
                'crates/viboceros-command/src/merge_surfaces/geometry.rs',
                'crates/viboceros-command/src/merge_surfaces/portable.rs',
                'src/app/object_selection/merge_surfaces.rs'}
    if not required.issubset(changed): raise ValueError('missing implementation')
    return dict(scope='Untrimmed MergeSrf command with automatic boundary orientation in portable and SMLib builds.',
        baseline_revision=BASELINE, implementation_revision=revision,
        geometry_observations=40, geometry_successes=38, geometry_declines=2,
        command_observations=20, command_successes=13, command_cancellations=5, command_failures=2,
        api_replays_per_build=15, ui_replays_per_build=20,
        retained_discovery_observations=6, retained_incomplete_command_observations=3, failed_capture_attempts=4,
        command_workflow='Two preselected surfaces execute immediately in document order. Command-first options precede two picks; the second pick executes in pick order. Cancel and failure retain preferences and pending redo. Numeric getters reject invalid ranges and accept fractions. The first UUID, attributes, groups and face sense survive; postselection clears. Undo/Redo and SelLast replay measured selection behavior.',
        geometry='Certify one complete affine boundary correspondence, orient the second operand to the first, elevate/refine compatible NURBS bases, merge homogeneous seam controls, and optionally spread seam knots. Construct one B-rep face and split boundary edges at G1-discontinuous seams. Native controls are expectations only.',
        comparison='Compare degrees, counts, knot clocks, face sense, 17 by 17 surface samples, complete boundary-curve certificates, vertices, metadata, source identity, history and 3DM round trips. API cancellation is excluded; application replay exercises all cancellations.',
        maximum_sampled_geometry_error=max(row.get('maximum_geometry_error',0.) for rows in stages.values() for row in rows),
        continuous_surface_distance_qualified=False, full_native_parity=False, performance_qualified=False,
        limits=['Trimmed, ambiguous, nonlinear boundary-speed and broader closed/periodic inputs remain unsupported or unqualified.',
                'Surface comparisons are finite parameter samples; joining tolerance does not bound the deliberate smoothing deformation.',
                'One failed owned-input attempt has no retained response; no native result was reconstructed for it. Other failed and incomplete attempts retain their original responses.'],
        captures=captures, stages=stages, checks=[dict(name=c['name'],status=c['status']) for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p)) for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(paths)})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--implementation-revision',required=True)
    args = parser.parse_args()
    (ROOT / 'docs/merge-srf-command-validation.json').write_text(
        json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
