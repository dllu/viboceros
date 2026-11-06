"""Rebuild the experimental two-view record from preserved validation logs."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
FAMILIES = ['', '_discovery', '_audit', '_fit_audit', '_surface_audit']


def build(proof, source_revision=None):
    def read(name):
        return gzip.decompress((proof / (name + '.log.gz')).read_bytes()).decode()

    checks = json.loads((proof / 'checks.json').read_text())
    required = {'geometry', 'closest-point', 'python', 'two-view-experiment',
                'clippy-portable', 'clippy-all-features', 'fmt'}
    required.update(kind + '-' + mode for kind in ['command', 'help', 'exact-audit',
                    'curve-boolean-command', 'curve-boolean-ui', 'planar-mesh-command', 'planar-mesh-ui']
                    for mode in ['portable', 'native'])
    if {c['name'] for c in checks} != required or len(checks) != len(required) or any(c['status'] != 0 for c in checks):
        raise ValueError('incomplete or failed validation')
    rows = [json.loads(s) for s in re.findall(r'TWO_VIEWS_CASE (\{[^\n]+\})', read('two-view-experiment'))]
    audits = [json.loads(s) for s in re.findall(r'TWO_VIEWS_EXACT_AUDIT (\{[^\n]+\})', read('exact-audit-portable'))]
    expected = json.loads((HERE / 'fixtures/curve_two_views.json').read_text())['operations']
    if [r['id'] for r in rows] != [o['id'] for o in expected if 'cancel' not in o]:
        raise ValueError('incomplete workflow coverage')
    if len(audits) != 5 or any(r['model_epsilon'] != 1e-6 or r['certificate']['excluded_radius'] != 4e-5 for r in audits):
        raise ValueError('incomplete exact exclusion audit')
    native_audits = [json.loads(s) for s in re.findall(r'TWO_VIEWS_EXACT_AUDIT (\{[^\n]+\})', read('exact-audit-native'))]
    if native_audits != audits:
        raise ValueError('exact exclusion audit differs across builds')
    if sum(r['geometry_qualified'] for r in rows) != 3 or sum(r['output_count_gap'] for r in rows) != 1:
        raise ValueError('experimental classification changed; review the native evidence')
    regressions = []
    for mode in ['portable', 'native']:
        for route, previous, marker in [('command', 'oracle', 'CASE'), ('ui', 'app', 'APP')]:
            old_path = ROOT / ('docs/curve-boolean-subcurves-proof/' + mode + '-curve-' + previous + '.log.gz')
            old = gzip.decompress(old_path.read_bytes()).decode()
            current = read('curve-boolean-' + route + '-' + mode)
            parse = lambda text: {r['id']: r for r in [json.loads(s) for s in re.findall(r'CURVE_BOOLEAN_' + marker + r' (\{[^\n]+\})', text)]}
            a, b = parse(old), parse(current)
            if len(a) != 83 or a != b:
                raise ValueError('CurveBoolean regression metrics changed: ' + mode + ' ' + route)
            regressions.append(dict(build=mode, route=route, workflows=83, all_recorded_metrics_unchanged=True))
    captures = []
    for suffix in FAMILIES:
        path = HERE / ('observations/curve_two_views' + suffix + '.json.gz')
        raw = gzip.decompress(path.read_bytes())
        data = json.loads(raw)
        captures.append(dict(path=str(path.relative_to(ROOT)), observations=len(data['results']),
                             raw_sha256=hashlib.sha256(raw).hexdigest(), engine_version=data['engine_version'],
                             invocation='bash tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_curve_two_views' + suffix,
                             provenance=data['provenance']))
    sources = {
        'Cargo.lock', 'crates/viboceros-oracle/Cargo.toml',
        'crates/viboceros-command/src/lib.rs',
        'crates/viboceros-geometry/src/lib.rs', 'crates/viboceros-geometry/src/error.rs',
        'crates/viboceros-geometry/src/nurbs.rs',
        'crates/viboceros-geometry/src/nurbs/closest_point.rs',
        'crates/viboceros-geometry/src/nurbs/closest_point/prepared.rs',
        'crates/viboceros-geometry/src/nurbs_surface/closest_point/tests.rs',
        'crates/viboceros-oracle/src/lib.rs',
        'crates/viboceros-oracle/tests/support/curve_boolean.rs',
        'tools/rhino_oracle/structural_sdk.py', 'tools/rhino_oracle/rhino_worker.py',
        'tools/rhino_oracle/test_curve_two_views.py',
        'tools/rhino_oracle/report_curve_two_views.py',
    }
    for pattern in ['crates/*/src/curve_two_views*.rs', 'crates/*/src/curve_two_views/*.rs',
                    'crates/viboceros-oracle/tests/curve_two_views*.rs',
                    'crates/viboceros-oracle/tests/support/curve_two_views*.rs',
                    'tools/rhino_oracle/curve_two_views*.py',
                    'tools/rhino_oracle/generate_curve_two_views*.py',
                    'tools/rhino_oracle/fixtures/curve_two_views*.json']:
        sources.update(str(p.relative_to(ROOT)) for p in ROOT.glob(pattern))
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    result = dict(
        scope='Experimental two-view geometry and immutable preparation; command intentionally unregistered.',
        baseline_revision='07aba3100ac71b56e7646aaea8c2e6f6de826d03', command_registered=False, model_epsilon=1e-6,
        full_native_parity=False, continuous_coverage_qualified=False, performance_qualified=False,
        execution='Native captures used private Xvfb displays. UI replays are windowless Rust unit tests.',
        native_observations=sum(c['observations'] for c in captures),
        local_workflows=len(rows), completed=sum(r['success'] for r in rows),
        sampled_gate_matches=dict(successful_constructions=1, expected_failures=2),
        above_gate_sampled_upper_bounds=19, output_count_gaps=1, native_only_cancellations=6,
        limits=[
            'A sampled distance upper bound above the model gate is inconclusive alone.',
            'The exact exclusion audit covers five serialized reference points against the entire independently known cubic, not every curved workflow.',
            'Equal circles have two local outputs and one native output; their union distance is not qualified.',
            'Cancellation controls have no local interactive replay. No GUI controller is registered.',
            'Curve fitting and direct surface-intersection controls do not establish the cause of the native discrepancies.',
        ],
        workflows=rows, exact_exclusions=audits, curve_boolean_regressions=regressions, captures=captures, checks=checks,
        source_sha256={p: sha(ROOT / p) for p in sorted(sources)},
        evidence_sha256={str(p.relative_to(ROOT)): sha(p) for p in sorted(proof.iterdir()) if p.is_file()},
    )
    if source_revision is not None:
        # This is a historical experiment. Later features must not relabel its
        # old test logs as validation of the current shared source files.
        if re.fullmatch(r'[0-9a-f]{40}', source_revision) is None:
            raise ValueError('full replay source revision required')
        result.pop('source_sha256')
        result['source_revision'] = source_revision
        result['source_git_sha256'] = {
            source_revision + ':' + p: hashlib.sha256(subprocess.run(
                ['git','cat-file','blob',source_revision+':'+p],cwd=ROOT,
                check=True,capture_output=True).stdout).hexdigest() for p in sorted(sources)}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof-dir', type=Path, default=ROOT / 'docs/curve-two-views-proof')
    parser.add_argument('--source-revision', default=None)
    args = parser.parse_args()
    (ROOT / 'docs/curve-two-views-validation.json').write_text(json.dumps(build(args.proof_dir,args.source_revision), indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
