"""Rebuild Plane qualification from immutable native captures and execution logs."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).parent
FAMILIES = {'plane_primitive_pilot': 8, 'plane_surface_workflows': 29,
            'plane_surface_audit': 10, 'plane_surface_holdouts': 12,
            'plane_surface_qualified': 51, 'plane_surface_terminal': 51,
            'plane_surface_guides': 5, 'plane_surface_vertical_audit': 7}
QUALIFIED = {'plane_surface_terminal', 'plane_surface_guides', 'plane_surface_vertical_audit'}
RECOVERED = {'deform_linear', 'audit_degree_one', 'audit_degree_twelve',
             'audit_low_count', 'audit_degree_then_count'}


def build(proof, implementation_revision):
    proof = proof.resolve()
    def read(path):
        raw = path.read_bytes()
        return gzip.decompress(raw) if path.suffix == '.gz' else raw
    def rows(name, prefix):
        text = read(proof / (name + '.log.gz')).decode()
        data = [json.loads(line) for line in re.findall(r'^' + prefix + r' (.+)$', text, re.M)]
        if len({r['id'] for r in data}) != len(data): raise ValueError('duplicate replay row: ' + name)
        return {r['id']: r for r in data}
    if not isinstance(implementation_revision, str) or re.fullmatch('[0-9a-f]{40}', implementation_revision) is None: raise ValueError('full implementation revision required')
    subprocess.run(['git', 'cat-file', '-e', implementation_revision + '^{commit}'], cwd=ROOT, check=True, capture_output=True)
    checks = json.loads((proof / 'checks.json').read_text())
    required = {'fmt', 'geometry', 'rectangle-input', 'registry', 'plane-primitives',
                'clippy-portable', 'clippy-all-features', 'python'}
    required.update(kind + '-' + mode for kind in ['commands', 'ui', 'rectangle', 'subd', 'surface-flow'] for mode in ['portable', 'native'])
    if len(checks) != len(required) or {c['name'] for c in checks} != required or any(c['status'] for c in checks):
        raise ValueError('incomplete or failed checks')
    evidence = set(proof.iterdir())
    runs = json.loads((proof/'captures.json').read_text())
    if len(runs) != len(FAMILIES) or {r['family'] for r in runs} != set(FAMILIES) or any(r['status'] != 0 or r['command'][:2] != ['tools/rhino_oracle/run_headless.sh','exec'] for r in runs): raise ValueError('incomplete headless captures')
    captures, native = [], {}
    for family, count in FAMILIES.items():
        path = HERE / ('observations/' + family + '.json.gz')
        raw = read(path); data = json.loads(raw)
        fixture = HERE / ('fixtures/' + family + '.json')
        query = json.loads(fixture.read_text())
        if len(data['results']) != count or [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]: raise ValueError('native coverage differs')
        if hashlib.sha256(fixture.read_bytes()).hexdigest() != data['provenance']['request_sha256']: raise ValueError('native recipe changed')
        evidence.update([path, fixture])
        evidence.update(HERE / ('observations/source_snapshots/' + digest + '.gz') for digest in data['provenance']['sources'].values())
        captures.append(dict(family=family, observations=count, raw_sha256=hashlib.sha256(raw).hexdigest(),
                             role='qualification' if family in QUALIFIED else 'exploration', engine_version=data['engine_version']))
        if family in QUALIFIED:
            for row in data['results']:
                if row['id'] in native: raise ValueError('duplicate native qualification case')
                native[row['id']] = row['value']
    if len(native) != 63 or sum(v['success'] for v in native.values()) != 53: raise ValueError('native scope changed')
    scripts = {k for k,v in native.items() if v['success'] and k not in RECOVERED}
    cases = {k: dict(id=k, success=v['success'], recovered_invalid_option=k in RECOVERED) for k,v in native.items()}
    for mode in ['portable', 'native']:
        cli = rows('commands-' + mode, 'PLANE_SCRIPT')
        ui = rows('ui-' + mode, 'PLANE_UI')
        if set(cli) != scripts or set(ui) != set(native): raise ValueError('incomplete CLI or UI execution')
        for key,v in native.items():
            for route, result in [('script', cli.get(key)), ('ui', ui[key])]:
                error = None if result is None else result['maximum_coefficient_and_sample_error']
                if error is not None and (type(error) not in (int,float) or not math.isfinite(error) or not 0 <= error <= 1e-6): raise ValueError('geometry gate failed: ' + key)
                cases[key][mode + '_' + route + '_error'] = error
            archived = ui[key]['archive_objects']
            expected = len(v['after']) if v['success'] else None
            if archived != expected: raise ValueError('archive coverage differs: ' + key)
            cases[key]['archive_objects'] = archived
    return dict(scope='Plane rectangle and guide input, deformable affine bases, source policy and atomic history.',
                baseline_revision='b2e698eea942f3a29732bde0ae4169c4d5d4eeca', implementation_revision=implementation_revision,
                model_epsilon=1e-6, full_native_parity=False, continuous_native_distance_qualified=False, performance_qualified=False,
                native_observations=sum(FAMILIES.values()), qualified_workflows=63, script_constructions=len(scripts),
                ui_constructions=53, ui_controls=10, recovered_invalid_option_workflows=5, archive_round_trips_per_build=53,
                maximum_coefficient_and_sample_difference=max(v for case in cases.values() for k,v in case.items() if k.endswith('_error') and v is not None),
                parameter_grid=[9,9], captures=captures, cases=list(cases.values()),
                execution='Native captures used private Xvfb displays. Rust command and application tests created no windows.',
                limits=['Earlier 110 observations are exploratory. Appended idle Cancel commands affected selection, runtime random recipes differed across Python hosts, and the first consolidated family did not resolve both issues.',
                        'The 63 qualified recipes use literal holdout coordinates, explicit native model tolerances and no idle cleanup in the measured macro.',
                        'CLI replays execute 48 valid constructions. Five recovered-invalid-setting constructions and ten cancellation/invalid-point workflows are UI-only.',
                        'Cancellation undo/redo fields are passive producer snapshots, not executed native history operations. Local tests separately check pending Redo preservation.',
                        'Native command-end telemetry retains temporary guide selection; post-script snapshots record its release. Persistent preselection and guide-picker cleanup are checked separately.',
                        '3DM preserves geometry, attributes and groups. Object selection is transient and is checked as cleared on archive restore.',
                        'Assignment-form Deformable=Yes|No tokens remain unsupported. The pilot preserves the native Deformable=No menu-switch result; current command syntax uses bare Deformable.',
                        'General curved guides, edge-frame parity, all Enter defaults, multiple preselected guide choices, mouse/pixel input, restart persistence, reactive History and comparative speed remain unqualified.'],
                checks=[dict(name=c['name'],status=c['status']) for c in checks],
                evidence_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(evidence) if p.is_file()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--proof-dir', type=Path, default=ROOT/'docs/plane-surface-proof')
    parser.add_argument('--implementation-revision', required=True)
    args = parser.parse_args()
    (ROOT/'docs/plane-surface-validation.json').write_text(json.dumps(build(args.proof_dir,args.implementation_revision),indent=2,allow_nan=False)+'\n')

if __name__ == '__main__': main()
