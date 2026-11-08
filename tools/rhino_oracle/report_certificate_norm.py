"""Revision-pinned continuous certificate arithmetic and offset regression evidence."""
import argparse
import gzip
import json
import math
import re
import subprocess
from .check_certificate_norm import ROOT,PROOF,commands
from .report_offset_srf_trims import comparisons,maximum_error
from .fillet_getter_evidence import sha

BASELINE='a7bc7da4c3ec667ea405c4ab6acad25c65037d2c'

def benchmark(proof=PROOF):
    text=gzip.decompress((proof/'benchmark.log.gz').read_bytes()).decode()
    rows=[json.loads(line.split(' ',1)[1]) for line in text.splitlines()
        if line.startswith('CERTIFICATE_NORM_BENCHMARK ')]
    if len(rows)!=1:raise ValueError('benchmark coverage')
    value=rows[0]
    if value['baseline_revision']!=BASELINE or [c['case'] for c in value['cases']]!=[0,1]:raise ValueError('benchmark coverage')
    for row,distance in zip(value['cases'],[-.3,.5]):
        if row['distance']!=distance or len(row['support_counts'])!=2 or any(type(n)is not int or n<4 for n in row['support_counts']):raise ValueError('benchmark inputs')
        for key in ['baseline_seconds','candidate_seconds']:
            if type(row[key])not in(int,float)or not math.isfinite(row[key])or row[key]<=0:raise ValueError('benchmark timing')
        if re.fullmatch('[0-9a-f]{64}',row['baseline_geometry_sha256'])is None or row['baseline_geometry_sha256']!=row['candidate_geometry_sha256']:raise ValueError('changed benchmark geometry')
    return value

def build(proof,revision):
    if re.fullmatch('[0-9a-f]{40}',revision or '')is None:raise ValueError('full implementation revision required')
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT)
    git('cat-file','-e',revision+'^{commit}');git('merge-base','--is-ancestor',BASELINE,revision)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [(c['name'],c['command']) for c in checks]!=list(commands())or any(c['status'] for c in checks):raise ValueError('incomplete checks')
    measurement=benchmark(proof)
    for group in ['current_source_sha256','producer_sha256']:
        for path,digest in measurement[group].items():
            if sha(git('cat-file','blob',revision+':'+path))!=digest:raise ValueError('benchmark source revision')
    cases,captures,counts,paths=comparisons(proof)
    paths.update(p for p in proof.iterdir()if p.is_file())
    changed=git('diff','--name-only','--diff-filter=ACMR',BASELINE,revision).decode().splitlines()
    if not {'crates/viboceros-geometry/src/surface_pullback/certificate/algebra.rs',
        'crates/viboceros-geometry/src/surface_pullback/certificate/interval.rs',
        'crates/viboceros-geometry/src/surface_pullback/certificate/algebra/tests.rs'}.issubset(changed):raise ValueError('missing implementation')
    text=gzip.decompress((proof/'geometry.log.gz').read_bytes()).decode()
    totals=re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)
    if totals is None:raise ValueError('geometry suite')
    return dict(scope='Outward interval screening of exact homogeneous coefficient norms.',
        baseline_revision=BASELINE,implementation_revision=revision,
        geometry_tests_passed=int(totals[1]),geometry_tests_ignored=int(totals[2]),**counts,
        native_supported=35,portable_supported=22,archive_round_trips=114,
        maximum_geometry_difference=maximum_error(cases,'maximum_geometry_error'),
        maximum_product_sampled_field_error=maximum_error(cases,'maximum_field_error'),
        maximum_native_sampled_field_error=maximum_error(cases,'native_maximum_field_error'),
        implementation='Exact rational coefficients convert to verified outward intervals. Scaled Euclidean norm intervals can certify within/outside; inconclusive, zero/subnormal limits and unavailable finite arithmetic retain the original exact rational fallback. Accepted square-root upper bounds are checked exactly. Tolerances, work/depth limits and certificate requirements are unchanged.',
        correctness='Independent exact squared inequalities test decisions and returned bounds across large denominators, signed homogeneous gauges, threshold neighbors, zero, subnormal and overflow ranges. Both benchmark variants receive identical authored trimmed Bezier sources and identical native fitted supports; complete resulting B-rep geometry/topology dumps must be byte-identical. Retained native captures remain expectations only; all API/UI metadata, history, preview and 3DM gates are replayed.',
        performance='Two debug-build complete surface edits compare the frozen baseline and current geometry crates in one process. Timings exclude native support fitting and include certificate rebuilding and B-rep validation. Results describe these cases and this machine; no whole-application or Rhino performance parity is claimed.',
        continuous_true_offset_bound=False,full_native_parity=False,rhino_performance_qualified=False,
        limits=['Finite offset comparison stations and prior geometry exclusions remain unchanged; native model-space edge parity is not separately qualified.',
            'The interval screen may be inconclusive and use the original exact path. Existing geometry tests marked ignored remain ignored.',
            'Benchmark timings are observational, not a hardware-independent speed guarantee.'],
        benchmark=measurement,captures=captures,cases=[dict(id=k,**cases[k])for k in sorted(cases)],
        checks=[dict(name=c['name'],status=c['status'])for c in checks],
        implementation_git_sha256={revision+':'+p:sha(git('cat-file','blob',revision+':'+p))for p in changed},
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--implementation-revision',required=True);args=parser.parse_args()
    (ROOT/'docs/certificate-norm-validation.json').write_text(json.dumps(build(PROOF,args.implementation_revision),indent=2,allow_nan=False)+'\n')
