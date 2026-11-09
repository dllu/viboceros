"""Owned refinement investigation; failed prototypes never qualify production."""
import copy,gzip,json,math,re
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
from . import match_surface_average_cases as sdk
PROOF=ROOT/'docs/surface-average-refinement-proof'
FAMILIES={
    'match_surface_average_refinement_pilot':sdk.REFINEMENT_PILOT,
    'match_surface_average_refinement_steps':sdk.REFINEMENT_STEPS,
    'match_surface_average_refinement_iso':sdk.REFINEMENT_ISO,
    'match_surface_average_refinement_tolerances':sdk.REFINEMENT_TOLERANCES,
    'match_surface_average_refinement_parameters':sdk.REFINEMENT_PARAMETERS,
    'match_surface_average_refinement_edge_parameters':sdk.REFINEMENT_EDGE_PARAMETERS,
}
BASELINE='1f7b68f500b952567df7f75ddd1f1ca329591f7f'
COMMAND=['cargo','test','-p','viboceros-command','--test','average_surface_refinement','--','--nocapture']
PROTOTYPE_SOURCES={
    'crates/viboceros-geometry/src/surface_match.rs',
    'crates/viboceros-geometry/src/surface_match/average.rs',
    'crates/viboceros-geometry/src/surface_match/refine.rs',
    'crates/viboceros-command/tests/support/surface_match.rs',
    'crates/viboceros-geometry/src/surface_match/average/refinement.rs',
    'crates/viboceros-command/tests/average_surface_refinement.rs',
}


def producers(family):
    return {n+'.py'for n in ['generate_'+family,'match_surface_average_cases','match_surface_average_sdk',
        'match_surface_cases','match_surface_sdk','structural_sdk','rhino_worker','client','brep_join_probe','cap_probe','capture_family']}


def launch(family,stem):
    return ['tools/rhino_oracle/run_headless.sh','exec','bash','-c','python3 -m tools.rhino_oracle.generate_'+family+
        ' > /tmp/match-average-'+stem+'.log 2>&1; capture_result=$?; echo "$capture_result" > /tmp/match-average-'+stem+'.status; exit "$capture_result"']


def complete(v):
    r=v['prescribed_case']
    if (not finite(v) or v['success']is not True or v['source_unchanged']is not True or len(v['inputs'])!=2
            or any(v[k]is None for k in ['output','target'])):raise ValueError('incomplete investigation outputs')
    configured(v)


def configured(v):
    r=v['prescribed_case']
    # Native EnableRefinement clamps the positional setting to ON_SQRT_EPSILON.
    expected=dict(enabled=r['refine'],distance=max(r.get('distance_tolerance',1e-6),1.490116119385e-8),angle=r.get('angle_tolerance',1e-6),curvature=r.get('curvature_tolerance',.05))
    settings=v['configured_settings']
    if (settings!=expected or type(settings['enabled'])is not bool
            or any(type(settings[k])not in (int,float)for k in ['distance','angle','curvature'])):raise ValueError('refinement readback mismatch')


def definition(s):
    return dict(type='surface',degree_u=s['degree'][0],degree_v=s['degree'][1],
        control_point_count_u=s['control_count'][0],control_point_count_v=s['control_count'][1],
        knots_u=s['knots_u'],knots_v=s['knots_v'],control_points=s['control_points'])


def composition(value):
    recipe=value['prescribed_case'];previous=value;calls=[]
    steps=value.get('average_pass_audit',[])
    if bool(steps)!=bool(recipe.get('average_pass_audit')) or len(steps)!=2:raise ValueError('missing averaging composition')
    for index,step in enumerate(steps):
        following=copy.deepcopy(recipe);following.pop('average_pass_audit')
        following.update(refine=False,reverse_match=not recipe['reverse_match'],insert_transverse_midpoints=True)
        following['sources']=[definition(previous[k]['surfaces'][0])for k in ['output','target']]
        if step['round']!=index+1:raise ValueError('averaging composition order')
        if step['status']=='invalid':
            if index!=1 or step['error']!='invalid surface match result' or step['prescribed_case']!=following:raise ValueError('unexpected diagnostic failure')
            calls.append(step);break
        if step['status']!='returned':raise ValueError('invalid composition status')
        current=step['value'];complete(current)
        if current['prescribed_case']!=following:raise ValueError('averaging composition inputs or flags')
        for source,original in zip(current['inputs'],following['sources']):
            s=source['surfaces'][0];knots=original['knots_v'];unique=sorted(set(knots))
            expected=sorted(knots+[a+(b-a)*.5 for a,b in zip(unique,unique[1:])])
            if s['knots_v']!=expected or s['knots_u']!=original['knots_u']:raise ValueError('diagnostic insertion knots')
        calls.append(step);previous=current
    return calls


def collect(proof=PROOF):
    paths={proof/'captures.json',proof/'attempts.json',HERE/'run_headless.sh'};rows={};receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('refinement capture inventory')
    for receipt,(family,recipes)in zip(receipts,FAMILIES.items()):
        stem=family.removeprefix('match_surface_average_').replace('_','-')+('-retry'if family.endswith('_steps')else'')
        if (receipt['archive']!='observations/'+family+'.json.gz' or receipt['command']!=launch(family,stem)
                or type(receipt['exit_status'])is not int or receipt['exit_status']!=0):raise ValueError('unowned refinement launch')
        archive=HERE/receipt['archive'];raw=gzip.decompress(archive.read_bytes());data=json.loads(raw);fixture=HERE/('fixtures/'+family+'.json');log=proof/receipt['log']
        if (sha(raw)!=receipt['observation_sha256'] or sha(fixture.read_bytes())!=receipt['request_sha256']
                or data['provenance']['request_sha256']!=receipt['request_sha256'] or json.loads(fixture.read_bytes())!=sdk.request(list(recipes))
                or data['engine']!='rhino' or data['iterations']!=1 or [r['id']for r in data['results']]!=list(recipes)
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=['Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]):raise ValueError('refinement capture receipt')
        if set(data['provenance']['sources'])!=producers(family):raise ValueError('refinement producer inventory')
        frozen(data['provenance'],paths);paths.update([archive,fixture,log])
        for row in data['results']:
            key,v=row['id'],row['value'];complete(v)
            if key in rows or v['prescribed_case']!=recipes[key]:raise ValueError('refinement authored recipe')
            if v['prescribed_case'].get('average_pass_audit'):composition(v)
            rows[key]=v
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=1:raise ValueError('refinement failed-launch inventory')
    for attempt in attempts:
        archive=HERE/attempt['archive'];raw=gzip.decompress(archive.read_bytes());value=json.loads(raw);family='match_surface_average_refinement_steps';log=proof/attempt['log']
        if (attempt['family']!=family or value['family']!=family or attempt['command']!=launch(family,'refinement-steps')
                or type(attempt['exit_status'])is not int or attempt['exit_status']!=1 or attempt['error']!=value['error']
                or 'invalid surface match result'not in value['error'] or not archive.name.endswith('-'+sha(raw)+'.json.gz')
                or value['provenance']['request_sha256']!=receipts[1]['request_sha256']
                or set(value['provenance']['sources'])!=producers(family)
                or value['error']not in gzip.decompress(log.read_bytes()).decode()):raise ValueError('refinement failed-launch receipt')
        frozen(value['provenance'],paths);paths.update([archive,log])
    if len(rows)!=102:raise ValueError('refinement observation scope')
    return rows,paths


def prototypes(proof=PROOF):
    paths=set();out={};expected=set(sdk.REFINEMENT_PILOT)
    for label,count in [('direct',25),('inverse',24)]:
        folder=proof/'prototypes';path=folder/(label+'.json');record=json.loads(path.read_bytes());log=folder/record['log'];text=gzip.decompress(log.read_bytes()).decode()
        expected_sources=PROTOTYPE_SOURCES|({'crates/viboceros-geometry/src/surface_match/fit.rs'}if label=='inverse'else set())
        if (record['baseline_revision']!=BASELINE or record['command']!=COMMAND or record['log']!=label+'.log.gz'
                or set(record['sources'])!=expected_sources or type(record['exit_status'])is not int or record['exit_status']!=101):raise ValueError('prototype is not an owned rejected replay')
        for name,digest in record['sources'].items():
            if not name.startswith('crates/') or not name.endswith('.rs') or re.fullmatch('[0-9a-f]{64}',digest)is None:raise ValueError('prototype source manifest')
            source=folder/'sources'/(digest+'.gz')
            if sha(gzip.decompress(source.read_bytes()))!=digest:raise ValueError('prototype source changed')
            paths.add(source)
        admitted=[s.removeprefix('AVERAGE_REFINEMENT_CASE ')for s in text.splitlines()if s.startswith('AVERAGE_REFINEMENT_CASE ')]
        results=[json.loads(s.removeprefix('AVERAGE_REFINEMENT '))for s in text.splitlines()if s.startswith('AVERAGE_REFINEMENT ')]
        failed=[json.loads(s.split('Unmatched native cases: ',1)[1])for s in text.splitlines()if 'Unmatched native cases: 'in s]
        passed={r['id']for r in results}
        if (set(admitted)!=expected or len(admitted)!=len(expected) or len(results)!=count or len(passed)!=count or len(failed)!=1
                or passed&set(failed[0]) or passed|set(failed[0])!=expected or len(failed[0])!=len(expected)-count
                or text.count('test result: FAILED.')!=1 or 'test result: ok.'in text):raise ValueError('prototype rejected inventory')
        for r in results:
            error=r['maximum_geometry_error']
            if type(error)not in (float,int)or not math.isfinite(error)or not 0<=error<=1e-9 or type(r['outputs'])is not int or r['outputs']!=2 or type(r['archives'])is not int or r['archives']!=2:raise ValueError('prototype comparison gate')
        out[label]=dict(backend='portable',cases=30,matched_cases=count,rejected_cases=sorted(failed[0]),maximum_matched_geometry_error=max(r['maximum_geometry_error']for r in results),qualified=False)
        paths.update([path,log])
    return out,paths
