"""Reproduce complete surface bounds against the owned averaging inventory."""
import gzip
import json
import math
import re

from .merge_surfaces_evidence import ROOT, sha
from .surface_average_curvature_evidence import collect

PROOF = ROOT / 'docs/surface-deviation-proof'
SOURCES = {
    'crates/viboceros-geometry/src/surface_pullback/certificate.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate/surface.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate/surface_pair.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate/surface_pair/tensor.rs',
    'crates/viboceros-geometry/src/surface_pullback/certificate/surface_pair/tests.rs',
    'crates/viboceros-command/tests/average_surfaces.rs',
}


def build(proof=PROOF):
    first, second, _, _, paths = collect()
    expected = set(first) | set(second)
    manifest = json.loads((proof / 'sources.json').read_bytes())
    if (manifest['baseline_revision'] != '25c906ad04626437d1fcfdbc0d347f951a4a54f7'
            or set(manifest['overrides']) != SOURCES):
        raise ValueError('surface bound source inventory')
    for name, digest in manifest['overrides'].items():
        if re.fullmatch('[0-9a-f]{64}', digest) is None:
            raise ValueError('surface bound source digest')
        source = proof / 'sources' / (digest + '.gz')
        if sha(gzip.decompress(source.read_bytes())) != digest:
            raise ValueError('surface bound frozen source changed')
        paths.add(source)
    checks = json.loads((proof / 'checks.json').read_bytes())
    if [check['label'] for check in checks] != ['certificates', 'portable', 'native']:
        raise ValueError('surface bound check inventory')
    backends = {}
    for check in checks:
        label = check['label']
        command = (['cargo', 'test', '-p', 'viboceros-geometry', '--lib', 'surface_pullback::certificate']
                   if label == 'certificates' else ['cargo', 'test', '-p', 'viboceros-command',
                       *(['--features', 'native-smlib'] if label == 'native' else []), '--test', 'average_surfaces'])
        if (check['command'] != [*command, '--', '--nocapture'] or check['log'] != label + '.log.gz'
                or type(check['exit_status']) is not int or check['exit_status'] != 0):
            raise ValueError('surface bound check receipt')
        log = proof / check['log']
        text = gzip.decompress(log.read_bytes()).decode()
        if 'test result: FAILED.' in text:
            raise ValueError('failed surface bound check')
        if label == 'certificates':
            if ('test result: ok. 93 passed; 0 failed;' not in text
                    or len(re.findall(r'^test surface_pullback::certificate::surface_pair::tests::\S+ \.\.\. ok$', text, re.M)) != 7):
                raise ValueError('surface bound unit test inventory')
        else:
            rows = [json.loads(line.removeprefix('SURFACE_PAIR_CERTIFICATE '))
                    for line in text.splitlines() if line.startswith('SURFACE_PAIR_CERTIFICATE ')]
            if (len(rows) != 119 or {row['id'] for row in rows} != expected
                    or 'test result: ok. 3 passed; 0 failed;' not in text):
                raise ValueError('complete surface bound replay inventory')
            for row in rows:
                bound = row['maximum_surface_bound']
                if (type(bound) not in (int, float) or not math.isfinite(bound) or not 0 <= bound <= 1e-9
                        or type(row['surfaces']) is not int or row['surfaces'] != 2
                        or type(row['archives']) is not int or row['archives'] != 2):
                    raise ValueError('surface bound comparison gate')
            backends[label] = dict(pairs=119, full_surface_bounds=476,
                                   maximum_surface_bound=max(row['maximum_surface_bound'] for row in rows))
        paths.add(log)
    paths.update([proof / 'sources.json', proof / 'checks.json'])
    return dict(scope='Complete normalized surface correspondence for the existing unrefined averaging cases.',
                certificate_tests=93, adversarial_surface_pair_tests=7, backends=backends,
                averaged_refinement_qualified=False, topology_qualified=False, performance_qualified=False,
                limits=['Bounds cover complete untrimmed tensor images with normalized U/V correspondence.',
                        'Degree, control count, sign, arithmetic, depth and work limits may return inconclusive.',
                        'This certifies the recorded cases and frozen sources; it does not qualify averaged refinement.'],
                evidence_sha256={str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(paths)})


if __name__ == '__main__':
    (ROOT / 'docs/surface-deviation-validation.json').write_text(json.dumps(build(), indent=2, allow_nan=False) + '\n')
    print('Recorded complete surface correspondence validation')
