"""Own all surface range captures, declined correspondences and backend checks."""
import gzip
import json
import math
import re
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
from .generate_surface_knot_reduction import request as primary
from .generate_surface_knot_reduction_holdouts import request as holdouts
from .surface_average_curvature_evidence import collect as averaging_cases

PROOF=ROOT/'docs/surface-knot-reduction-proof'
FAMILIES={'surface_knot_reduction':(primary,'surface-knot-reduction-complete'),
          'surface_knot_reduction_holdouts':(holdouts,'surface-knot-reduction-holdouts')}
SOURCES={
    'crates/viboceros-geometry/src/'+name for name in ['error.rs','lib.rs','nurbs.rs','nurbs/knot_reduction.rs','nurbs_surface.rs',
        'nurbs_surface/knot_reduction.rs','nurbs_surface/knot_reduction/basis.rs','nurbs_surface/knot_reduction/tests.rs',
        'nurbs_surface/knot_reduction/backend_tests.rs','surface_pullback/certificate/surface_pair.rs','surface_pullback/certificate/surface_pair/tensor.rs']}
SOURCES.update('crates/viboceros-oracle/'+name for name in ['src/lib.rs','src/surface_knot_reduction.rs','tests/surface_knot_reduction_native.rs'])
SOURCES.update('crates/viboceros-smlib/src/'+name for name in ['lib.rs','native.rs','native/knot_refinement.rs'])


def launch(family,stem):
    return ['tools/rhino_oracle/run_headless.sh','exec','bash','-c','python3 -m tools.rhino_oracle.generate_'+family+
        ' > /tmp/'+stem+'.log 2>&1; capture_result=$?; echo "$capture_result" > /tmp/'+stem+'.status; exit "$capture_result"']


def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes());rows={}
    paths={proof/'captures.json',proof/'attempts.json',HERE/'run_headless.sh'}
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('surface reduction capture inventory')
    for receipt,(family,(factory,stem))in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/receipt['archive'];raw=gzip.decompress(archive.read_bytes());data=json.loads(raw);log=proof/receipt['log']
        if (receipt['command']!=launch(family,stem) or type(receipt['exit_status'])is not int or receipt['exit_status']!=0
                or receipt['archive']!='observations/'+family+'.json.gz' or receipt['log']!='capture-'+family+'.log.gz'
                or sha(raw)!=receipt['observation_sha256'] or sha(fixture.read_bytes())!=receipt['request_sha256']
                or data['provenance']['request_sha256']!=receipt['request_sha256'] or json.loads(fixture.read_bytes())!=factory()
                or data['engine']!='rhino' or data['iterations']!=1
                or [r['id']for r in data['results']]!=[o['id']for o in factory()['operations']]
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(data['results']))+' cases: '+family,'Recorded '+family]):raise ValueError('surface reduction capture receipt')
        expected={'generate_'+family+'.py','surface_knot_reduction_probe.py','rhino_worker.py','client.py','structural_sdk.py','capture_family.py'}
        if set(data['provenance']['sources'])!=expected:raise ValueError('surface reduction producer inventory')
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        for operation,row in zip(factory()['operations'],data['results']):
            value=row['value']
            if (row['id']in rows or not finite(value) or value['before']!=value['after'] or len(value['samples'])!=81
                    or type(value['removed'])is not int or value['removed']<0):raise ValueError('surface reduction owned source')
            rows[row['id']]=(operation,value)
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=2:raise ValueError('surface reduction failed-launch inventory')
    for attempt,stem,error in zip(attempts,['surface-knot-reduction','surface-knot-reduction-retry'],['isfinite','DuplicateSurface']):
        archive=HERE/attempt['archive'];raw=gzip.decompress(archive.read_bytes());data=json.loads(raw);log=proof/attempt['log']
        if (attempt['family']!='surface_knot_reduction' or data['family']!=attempt['family'] or type(attempt['exit_status'])is not int or attempt['exit_status']!=1
                or attempt['command']!=launch('surface_knot_reduction',stem) or data['error']!=attempt['error'] or error not in data['error']
                or not archive.name.endswith('-'+sha(raw)+'.json.gz') or data['provenance']['request_sha256']!=receipts[0]['request_sha256']
                or data['error']not in gzip.decompress(log.read_bytes()).decode()):raise ValueError('surface reduction failed-launch receipt')
        frozen(data['provenance'],paths);paths.update([archive,log])
    if len(rows)!=88:raise ValueError('surface reduction primary scope')
    return rows,paths


def build(proof=PROOF):
    rows,paths=collect(proof);manifest=json.loads((proof/'sources.json').read_bytes())
    if (manifest['baseline_revision']!='cdc1888b03528293650e39ab6e9b2d3953207788' or set(manifest['overrides'])!=SOURCES
            or set(manifest['python_sources'])!={'tools/rhino_oracle/surface_knot_reduction_probe.py','tools/rhino_oracle/test_surface_knot_reduction.py'}):raise ValueError('surface reduction code inventory')
    for name,digest in {**manifest['overrides'],**manifest['python_sources']}.items():
        if re.fullmatch('[0-9a-f]{64}',digest)is None:raise ValueError('surface reduction source name')
        path=proof/'sources'/(digest+'.gz')
        if sha(gzip.decompress(path.read_bytes()))!=digest:raise ValueError('surface reduction frozen source changed')
        paths.add(path)
    checks=json.loads((proof/'checks.json').read_bytes())
    if [r['label']for r in checks]!=['kernel','certificates','python','portable','native']:raise ValueError('surface reduction check inventory')
    commands={
        'kernel':['cargo','test','-p','viboceros-geometry','--lib','knot_reduction','--','--nocapture'],
        'certificates':['cargo','test','-p','viboceros-geometry','--lib','surface_pullback::certificate','--','--nocapture'],
        'python':['python3','-m','unittest','tools.rhino_oracle.test_surface_knot_reduction'],
        'portable':['cargo','test','-p','viboceros-oracle','--test','surface_knot_reduction_native','--','--nocapture'],
        'native':['cargo','test','-p','viboceros-oracle','--features','native-smlib','--test','surface_knot_reduction_native','--','--nocapture']}
    backends={}
    for receipt in checks:
        label=receipt['label'];log=proof/receipt['log'];text=gzip.decompress(log.read_bytes()).decode()
        if (receipt['command']!=commands[label]or receipt['log']!=label+'.log.gz'
                or type(receipt['exit_status'])is not int or receipt['exit_status']!=0 or 'FAILED'in text):raise ValueError('surface reduction check receipt')
        if label in ['kernel','certificates']:
            count=18 if label=='kernel'else 93
            if 'test result: ok. '+str(count)+' passed; 0 failed;'not in text:raise ValueError('surface reduction unit checks')
        elif label=='python':
            if 'Ran 3 tests'not in text or not text.rstrip().endswith('OK'):raise ValueError('surface reduction Python checks')
        else:
            results=[json.loads(s.removeprefix('SURFACE_REDUCTION '))for s in text.splitlines()if s.startswith('SURFACE_REDUCTION ')]
            if len(results)!=88 or {r['id']for r in results}!=set(rows)or 'test result: ok. 2 passed; 0 failed;'not in text:raise ValueError('surface reduction backend scope')
            matched=[];declined=[];maximum=0.;archive_maximum=0.
            for r in results:
                operation,native=rows[r['id']];limit=operation['absolute_tolerance'];bound=r['source_bound'];error=r['maximum_native_bound'];archive_bound=r['maximum_archive_bound']
                if (type(r['native_match'])is not bool or type(r['removed'])is not int or type(r['native_removed'])is not int or r['native_removed']!=native['removed']
                        or type(archive_bound)not in(int,float)or not math.isfinite(archive_bound)or not 0<=archive_bound<=1e-9
                        or limit==0. and bound is not None or limit>0. and(type(bound)not in(int,float)or not math.isfinite(bound)or not 0<=bound<=limit)):raise ValueError('surface reduction positional gate')
                archive_maximum=max(archive_maximum,archive_bound)
                if r['native_match']:
                    if r['removed']!=native['removed']or type(error)not in(int,float)or not math.isfinite(error)or not 0<=error<=1e-9:raise ValueError('surface reduction native match gate')
                    matched.append(r['id']);maximum=max(maximum,error)
                else:
                    if limit<=0. or error is not None or not 0<=r['removed']<native['removed']:raise ValueError('surface reduction declined edit gate')
                    declined.append(r['id'])
            if (len(matched),len(declined))!=(64,24):raise ValueError('surface reduction classification scope')
            backends[label]=dict(matched=64,declined=24,cases=88,maximum_native_bound=maximum,
                maximum_archive_bound=archive_maximum,declined_cases=sorted(declined))
        paths.add(log)
    regression=json.loads((proof/'averaging.json').read_bytes())
    if [r['label']for r in regression]!=['portable','native']:raise ValueError('averaging regression check inventory')
    first,second,_,_,average_paths=averaging_cases();paths.update(average_paths);expected=set(first)|set(second)
    averages={}
    for receipt in regression:
        label=receipt['label'];log=proof/receipt['log'];text=gzip.decompress(log.read_bytes()).decode()
        command=['cargo','test','-p','viboceros-command',*(['--features','native-smlib']if label=='native'else[]),'--test','average_surfaces','--','--nocapture']
        results=[json.loads(s.removeprefix('SURFACE_PAIR_CERTIFICATE '))for s in text.splitlines()if s.startswith('SURFACE_PAIR_CERTIFICATE ')]
        if (receipt['command']!=command or receipt['log']!='averaging-'+label+'.log.gz' or type(receipt['exit_status'])is not int or receipt['exit_status']!=0
                or len(results)!=119 or {r['id']for r in results}!=expected or 'test result: ok. 3 passed; 0 failed;'not in text):raise ValueError('averaging full-surface regression scope')
        for r in results:
            bound=r['maximum_surface_bound']
            if (type(bound)not in(int,float)or not math.isfinite(bound)or not 0<=bound<=1e-9
                    or r['surfaces']!=2 or r['archives']!=2):raise ValueError('averaging full-surface regression gate')
        averages[label]=dict(pairs=119,maximum_surface_bound=max(r['maximum_surface_bound']for r in results));paths.add(log)
    paths.update([proof/'sources.json',proof/'checks.json',proof/'averaging.json'])
    return dict(scope='Tolerance-bounded tensor knot reduction under complete normalized parameter correspondence.',
        native_observations=88,successful_headless_launches=2,retained_failed_launches=2,
        kernel_tests=18,certificate_regression_tests=93,python_probe_tests=3,backends=backends,averaging_regression=averages,
        full_rhino_parity=False,performance_qualified=False,
        limits=['Declined cases are not counted as Native geometry matches.',
            'The normalized parameter bound is stronger than geometric image agreement; failure to certify does not prove geometric disagreement.',
            'The SDK comparison protocol maps zero to unrestricted edits; Rust Some(0) stays strict.',
            'No interactive command or trimming/topology qualification is claimed.'],
        evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    (ROOT/'docs/surface-knot-reduction-validation.json').write_text(json.dumps(build(),indent=2,allow_nan=False)+'\n')
    print('Recorded bounded surface knot reduction validation')
