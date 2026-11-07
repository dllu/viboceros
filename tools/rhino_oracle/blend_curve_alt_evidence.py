"""Keep nominal Alt workflows distinct from nonzero-exit pilot replays."""
import gzip,hashlib,json
from . import blend_curve_command as recipes,blend_curve_surface_high_evidence as prior
from .blend_curve_high_evidence import captures as read_captures
from .blend_curve_evidence import command_result,normalized,output
from .arc_blend_radius_evidence import finite,read,HERE
from .blend_curve_alt_policy import characterize
from .check_blend_curve_alt import ROOT
BASELINE='be9b06876a8f2c3b85000ea6bc35cdead5cadce1'
FAMILIES=dict(blend_curve_alt_verification=list(recipes.ALT_VERIFICATION),blend_curve_alt_followups=list(recipes.ALT_FOLLOWUPS),blend_curve_alt_picking=list(recipes.ALT_PICKING),blend_curve_alt_selection=list(recipes.ALT_SELECTION))
SECONDARY='blend_curve_alt_selection/plain_alt_far_surface'
sha=lambda b:hashlib.sha256(b).hexdigest()
def captures(proof):
    rows,summary,paths=read_captures(proof,FAMILIES)
    receipts=json.loads((proof/'captures.json').read_bytes())
    for receipt in receipts:
        family=receipt['family'];raw=gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes())
        if sha(raw)!=receipt['observation_sha256']or sha((HERE/('fixtures/'+family+'.json')).read_bytes())!=receipt['request_sha256']:raise ValueError('Alt capture digest changed')
    return rows,summary,paths
def pilot(proof):
    receipts=json.loads((proof/'diagnostics.json').read_bytes())
    if len(receipts)!=1:raise ValueError('Alt diagnostic capture inventory changed')
    receipt=receipts[0];family='blend_curve_alt_pilot';p=HERE/('observations/'+family+'.json.gz');request=HERE/('fixtures/'+family+'.json');raw=gzip.decompress(p.read_bytes());data=json.loads(raw);paths={p,request,proof/receipt['log']}
    if (receipt['family']!=family or receipt['exit_status']!=143 or receipt['observations']!=8 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]
        or sha(raw)!=receipt['observation_sha256']or sha(request.read_bytes())!=receipt['request_sha256']or sha(request.read_bytes())!=data['provenance']['request_sha256']):raise ValueError('Alt diagnostic terminal receipt changed')
    if (data['engine']!='rhino'or data['iterations']!=1 or [r['id']for r in data['results']]!=list(recipes.ALT_PILOT)
        or json.loads(request.read_bytes())!=recipes.request(list(recipes.ALT_PILOT))or gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()!=['Capturing 8 cases: '+family,'Recorded '+family]):raise ValueError('Alt diagnostic coverage changed')
    for digest in data['provenance']['sources'].values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('Alt diagnostic frozen producer changed')
        paths.add(p)
    return {family+'/'+r['id']:r['value']for r in data['results']},paths
def collect(proof):
    nominal,summary,paths=captures(proof);diagnostic,extra=pilot(proof);paths.update(extra)
    *_,inherited=prior.collect(ROOT/'docs/blend-curve-surface-high-proof');paths.update(inherited);paths.add(ROOT/'docs/blend-curve-surface-high-validation.json')
    all_rows=diagnostic|nominal;workflows={k:v for k,v in nominal.items()if k!=SECONDARY};replays=diagnostic|workflows
    picks=alt=shift=stations=derivatives=joins=cancelled=0
    for key,v in all_rows.items():
        name=key.split('/')[1];trace=v['input_trace'][0];recipe=recipes.CASES[name]
        if not prior.recipe_matches(v['prescribed_case'],recipe)or not v['returned_idle']or not finite(v):raise ValueError('Alt recipe or finite geometry changed')
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions']):raise ValueError('incomplete Alt physical workflow')
        if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid Native Alt geometry')
        accepted=command_result(v)=='Nothing'
        if v['success']is not False or v['history_changed']is not accepted or command_result(v)not in ['Nothing','Cancel']:raise ValueError('Alt Native command result changed')
        if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('Alt native history changed')
        o=output(v)
        if accepted:
            if o is None or len(o['samples'])!=33:raise ValueError('missing Alt Native stations')
            if o['guide']:
                source=next(x for x in v['before']if x['attributes']['name']==o['attributes']['name'])
                if 'brep'in source or o['attributes']!=source['attributes']:raise ValueError('Alt join ownership changed')
            elif o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']:raise ValueError('Alt output attributes changed')
            if key in replays:stations+=33;joins+=o['native_type']=='PolyCurve'
        else:
            if o is not None or normalized(v['before'])!=normalized(v['after'])or v['pending_redo']is None:raise ValueError('Alt cancellation changed history')
            cancelled+=key in replays
        for before in v['before']:
            if 'brep'in before and before!=next(x for x in v['after']if x['attributes']['name']==before['attributes']['name']):raise ValueError('Alt surface parent changed')
        if trace['alt_observation']!='System.Windows.Forms.Control.ModifierKeys sampled in the public mouse-down callback':raise ValueError('missing public Alt observation source')
        for c in trace['clicks']:
            a=recipe['actions'][c['action']]
            if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or c.get('shift_key_down')is not bool(a.get('shift',False))or c.get('alt_key_down')is not bool(a.get('alt',False)):raise ValueError('missing public Alt/Shift physical receipt')
            if 'edge_pick_context'in c:prior.prior.nearest(c)
            if key in replays:picks+=1;alt+=c['alt_key_down'];shift+=c['shift_key_down']
        for phase in ['before','after']:
            for x in v[phase]:
                if 'curve'not in x:continue
                samples=x.get('derivative_samples',[])
                if len(samples)!=3 or any(len(s['derivatives'])!=5 for s in samples):raise ValueError('missing Alt fourth-order blend derivatives')
                if key in replays:derivatives+=3
    policy=characterize(all_rows)
    if policy!=json.loads((proof/'construction-policy.json').read_bytes()):raise ValueError('Alt construction policy changed')
    attempts=json.loads((proof/'validation-attempts.json').read_bytes())
    if len(attempts)!=2 or attempts[0]['status']!=101 or 'no method named `add`'not in gzip.decompress((proof/attempts[0]['log']).read_bytes()).decode():raise ValueError('initial Alt UI compile receipt changed')
    if attempts[1]['status']!=1 or 'capture inventory changed'not in gzip.decompress((proof/attempts[1]['log']).read_bytes()).decode():raise ValueError('intermediate Alt inventory audit receipt changed')
    lint=json.loads((proof/'lint-attempts.json').read_bytes())
    if len(lint)!=2:raise ValueError('Alt lint attempt inventory changed')
    for row,needle in zip(lint,['function `handle` is never used','function `tight_length_error` is never used']):
        if row['status']!=101 or needle not in gzip.decompress((proof/row['log']).read_bytes()).decode():raise ValueError('initial Alt lint receipt changed')
    if (len(all_rows),len(workflows),len(replays),joins,cancelled)!=(46,37,45,1,1):raise ValueError('Alt workflow classification inventory changed')
    return workflows,replays,summary,dict(physical_picks=picks,alt_clicks=alt,shift_clicks=shift,point_tangent_stations=stations,derivative_stations=derivatives,joined_workflows=joins,cancelled_workflows=cancelled),paths
