"""Discovery-only tolerance, refinement and public-query observations."""
import gzip,json
from . import match_surface_cases as sdk,match_surface_command as command
from .merge_surfaces_evidence import ROOT,HERE,sha,frozen
from .fillet_getter_evidence import finite
PROOF=ROOT/'docs/surface-match-tolerance-evidence'
# Order follows the completed owned launches; no fitting comparison is qualified.
FAMILIES={
 'match_surface_command_tolerance_discovery':('command','TOLERANCE_DISCOVERY'),
 'match_surface_tolerances_sdk':('sdk','TOLERANCE_HOLDOUTS'),
 'match_surface_command_tolerance_getters':('command','TOLERANCE_GETTERS'),
 'match_surface_command_tolerance_matches':('command','TOLERANCE_MATCHES'),
 'match_surface_tolerance_sweep':('sdk','TOLERANCE_SWEEP'),
 'match_surface_command_tolerance_defaults':('command','TOLERANCE_DEFAULTS'),
 'match_surface_closest_audit':('sdk','CLOSEST_AUDIT'),
 'match_surface_command_tolerance_first_custom':('command','TOLERANCE_FIRST_CUSTOM'),
 'match_surface_local_closest_audit':('sdk','LOCAL_CLOSEST_AUDIT'),
 'match_surface_local_query_audit':('sdk','LOCAL_QUERY_AUDIT'),
 'match_surface_tolerance_mirrors':('sdk','TOLERANCE_MIRRORS'),
 'match_surface_inverse_query_audit':('sdk','INVERSE_QUERY_AUDIT'),
 'match_surface_gap_sweep':('sdk','GAP_SWEEP'),
 'match_surface_deviation_query_audit':('sdk','DEVIATION_QUERY_AUDIT'),
}


def collect(proof=PROOF):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('tolerance investigation launch inventory')
    rows={};paths={proof/'captures.json',HERE/'run_headless.sh'};captures=[]
    for receipt,(family,(kind,symbol)) in zip(receipts,FAMILIES.items()):
        factory=command if kind=='command' else sdk;recipes=getattr(factory,symbol)
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz');log=proof/receipt['log']
        stem=receipt['log'].removeprefix('capture-').removesuffix('.log.gz')
        launch=['tools/rhino_oracle/run_headless.sh','exec','bash','-c',
            'python3 -m tools.rhino_oracle.generate_'+family+' > /tmp/match-tolerance-'+stem+'.log 2>&1; '
            'capture_result=$?; echo "$capture_result" > /tmp/match-tolerance-'+stem+'.status; exit "$capture_result"']
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        if (type(receipt['exit_status']) is not int or receipt['exit_status']!=0
                or receipt['command']!=launch
                or receipt['archive']!='observations/'+family+'.json.gz'
                or sha(raw)!=receipt['observation_sha256']
                or sha(fixture.read_bytes())!=receipt['request_sha256']
                or data['provenance']['request_sha256']!=receipt['request_sha256']
                or json.loads(fixture.read_bytes())!=factory.request(list(recipes))
                or [r['id']for r in data['results']]!=list(recipes)
                or data['engine']!='rhino' or data['iterations']!=1
                or gzip.decompress(log.read_bytes()).decode().splitlines()!=[
                    'Capturing '+str(len(recipes))+' cases: '+family,'Recorded '+family]):raise ValueError('tolerance investigation capture receipt')
        required={n+'.py'for n in ['generate_'+family,'match_surface_cases','structural_sdk','rhino_worker','client','capture_family']}
        required.update(n+'.py'for n in (['match_surface_sdk','brep_join_probe','cap_probe'] if kind=='sdk' else [
            'arc_point_picker','catenary_point_picker','chamfer_point_picker','subcurve_direction_input','group_picking',
            'match_surface_point_picker','match_surface_command_probe','match_surface_command','line_scene_probe',
            'join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token']))
        if set(data['provenance']['sources'])!=required:raise ValueError('tolerance investigation producer inventory')
        frozen(data['provenance'],paths);paths.update([fixture,archive,log])
        captures.append(dict(family=family,kind=kind,observations=len(recipes),exit_status=0,engine_version=data['engine_version']))
        for row in data['results']:
            key,v=row['id'],row['value']
            if key in rows or v['prescribed_case']!=recipes[key] or not finite(v):raise ValueError('tolerance investigation authored row')
            if kind=='command':
                trace=v['input_trace'][0]
                if (trace['completed']!=trace['expected'] or trace['expected']!=len(recipes[key]['actions'])
                        or trace['errors'] or not v['returned_idle']
                        or any(not c['motion_acknowledged'] or not c['click_acknowledged']for c in trace.get('clicks',[]))):raise ValueError('incomplete owned tolerance command')
            elif not v['success'] or not v['source_unchanged'] or v['output'] is None:raise ValueError('incomplete owned tolerance SDK result')
            rows[key]=v
    if len(rows)!=78 or sum(r['observations']for r in captures if r['kind']=='command')!=30:raise ValueError('tolerance observation scope')
    return rows,captures,paths


def summary():
    rows,captures,paths=collect()
    definitions=lambda v:v['output']['surfaces'][0]['control_count']
    audits=[v['audit']for v in rows.values()if 'audit' in v]
    samples=[s for a in audits for s in a['samples']]
    return dict(scope='Native investigation evidence; custom tolerance fitting remains unqualified.',
        native_observations=len(rows),owned_launches=len(captures),command_rows=30,sdk_rows=48,custom_fitting_qualified=False,
        separately_qualified_menu_rows=['match_tol_complete_menus'],
        instrumentation_defect='Three local_closest_audit rows executed local queries but overwrote their fields. They remain diagnostic; the later local_query_audit launch retains the queries.',
        observations=[
            'Fresh private schemes display initial tolerance defaults 0.001 model units, 1 degree and 0.05 percent. Displayed model defaults lag subsequent document changes; explicit edits survive cancellation.',
            'New getter entries require position >=1e-5, tangency >=0.1 degrees and curvature >=the decimal ON_SQRT_EPSILON. Empty Enter can retain a smaller displayed model default.',
            'Changing only curvature tolerance from 1e-5 to 100 percent leaves the four captured loose-angle surfaces identical.',
            'A loose source fit can keep four transverse controls; a tighter fit can need six. More stringent positional sweeps produce five or thirteen even for a boundary that is geometrically coincident after matching.',
            'Public global/local curve closest-point, both surface closest-point directions and curve deviation queries report near-machine-precision agreement in the retained audit cases.'],
        sdk_density={k:definitions(v)for k,v in rows.items()if k.startswith(('match_tol_sweep_','match_tol_mirror_','match_tol_gap_','match_tol_sdk_','match_tol_already_matched_'))},
        public_query_max_distance=dict(
            curve_closest=max(s['distance']for s in samples),
            **{field:max(s[field]['distance']for s in samples if field in s)for field in [
                'local_closest','surface_closest','inverse_surface_closest']},
            curve_deviation=max(q['values'][0]for a in audits for q in a.get('curve_deviations',[])if q['success'])),
        captures=captures,evidence_sha256={str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)})


if __name__=='__main__':
    path=ROOT/'docs/surface-match-tolerance-investigation.json'
    path.write_text(json.dumps(summary(),indent=2,allow_nan=False)+'\n');print('Recorded unqualified surface-match tolerance investigation')
