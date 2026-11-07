"""Keep complete control workflows separate from timed-out numeric investigations."""
import gzip,hashlib,json,math
from . import blend_curve_command as recipes,blend_curve_alt_evidence as prior
from .blend_curve_evidence import command_result,normalized,output
from .arc_blend_radius_evidence import finite,read,HERE
from .check_blend_curve_controls import ROOT
from .blend_curve_high_evidence import orders
from .blend_curve_length_audit import measure
BASELINE='3014fba03185c190824f0f67fdff6015d19e9f69'
FAMILIES={
 'blend_curve_control_discovery':list(recipes.CONTROL_DISCOVERY),
 'blend_curve_control_policy':list(recipes.CONTROL_POLICY),
 'blend_curve_control_calibration':list(recipes.CONTROL_CALIBRATION),
 'blend_curve_control_holdouts':list(recipes.CONTROL_HOLDOUTS),
 'blend_curve_control_validation':list(recipes.CONTROL_VALIDATION),
 'blend_curve_control_numeric':list(recipes.CONTROL_NUMERIC),
 'blend_curve_control_symmetry':list(recipes.CONTROL_SYMMETRY),
 'blend_curve_control_lengths':list(recipes.CONTROL_LENGTHS),
}
DIAGNOSTICS={'blend_curve_control_discovery/control_secondary_numeric','blend_curve_control_policy/control_numeric_plane','blend_curve_control_policy/control_numeric_curved'}
DOMAIN='blend_curve_control_validation/control_validate_rational_g4'
LENGTH='blend_curve_control_lengths/control_validate_rational_g4_lengths'
sha=lambda b:hashlib.sha256(b).hexdigest()
def captures(proof):
 receipts=json.loads((proof/'captures.json').read_bytes());rows={};summary=[];paths={p for p in proof.iterdir()if p.is_file()}
 if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('control capture inventory changed')
 for receipt,(family,names)in zip(receipts,FAMILIES.items()):
  path=HERE/('observations/'+family+'.json.gz');fixture=HERE/('fixtures/'+family+'.json');raw=gzip.decompress(path.read_bytes());data=json.loads(raw);paths.update([path,fixture])
  if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]:raise ValueError('control terminal receipt changed')
  if sha(raw)!=receipt['observation_sha256']or sha(fixture.read_bytes())!=receipt['request_sha256']or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('control capture digest changed')
  if data['engine']!='rhino'or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names):raise ValueError('control fixed request changed')
  lines=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines();expected=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]
  if lines[-2:]!=expected or (lines!=expected and not(family=='blend_curve_control_calibration'and len(lines)==4 and 'SyntaxWarning: invalid decimal literal'in lines[0])):raise ValueError('control capture log changed')
  for digest in data['provenance']['sources'].values():
   p=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('control frozen producer changed')
   paths.add(p)
  summary.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(raw)))
  rows.update((family+'/'+r['id'],r['value'])for r in data['results'])
 return rows,summary,paths
def collect(proof):
 rows,summary,paths=captures(proof)
 attempts=json.loads((proof/'validation-attempts.json').read_bytes())
 expected=[('controls-command-initial',101,'control_holdout_g3_numeric'),('controls-ui-initial',101,'control_holdout_g3_numeric'),('controls-command-domain-attempt',101,'ratio=4.547395505558622e0'),('evidence-initial',1,'fourth control policy changed'),('alt-ui-legacy-expectation',101,'distant_click_selects_the_nearest_projected_control'),('clippy-portable-initial',101,'needless_range_loop')]
 if [(a['name'],a['status'])for a in attempts]!=[(n,s)for n,s,_ in expected]or any(needle not in gzip.decompress((proof/a['log']).read_bytes()).decode()for a,(_,_,needle)in zip(attempts,expected)):raise ValueError('control validation attempt receipts changed')
 *_,inherited=prior.collect(ROOT/'docs/blend-curve-alt-proof');paths.update(inherited);paths.add(ROOT/'docs/blend-curve-alt-validation.json')
 old,_,_=prior.captures(ROOT/'docs/blend-curve-alt-proof');rows[prior.SECONDARY]=old[prior.SECONDARY]
 lengths=rows.pop(LENGTH);length_output=output(lengths);source=output(rows[DOMAIN]);audit=measure(source['curve']);sdk=length_output['length_measurements']
 if (not prior.prior.recipe_matches(lengths['prescribed_case'],recipes.CASES[LENGTH.split('/')[1]])or lengths['input_trace'][0]['errors']or lengths['input_trace'][0]['completed']!=lengths['input_trace'][0]['expected']or command_result(lengths)!='Nothing'or length_output['curve']!=source['curve']):raise ValueError('control length diagnostic changed')
 if (sdk['default']!=audit['native_domain_span']or sdk['fractional']!=sdk['unit_domain_fractional']or [r['tolerance']for r in sdk['fractional']]!=[1e-6,1e-8,1e-10,1e-12]or abs(sdk['fractional'][-1]['length']-audit['independent_length'])>1e-9 or audit['native_domain_minus_length']<=max(2e-6,1e-8*audit['native_domain_span'])):raise ValueError('control domain approximation diagnostic changed')
 audit['public_sdk']=sdk
 workflows={k:v for k,v in rows.items()if k not in DIAGNOSTICS}
 picks=shift=stations=derivatives=cancelled=joins=0;fourth_errors=[]
 for key,v in rows.items():
  recipe=recipes.CASES[key.split('/')[1]];trace=v['input_trace'][0]
  if not prior.prior.recipe_matches(v['prescribed_case'],recipe)or not v['returned_idle']or not finite(v):raise ValueError('control recipe or finite geometry changed')
  if key in DIAGNOSTICS:
   if 'owned BlendCrv input timeout'not in trace['errors']or v['history_changed']or normalized(v['before'])!=normalized(v['after'])or command_result(v)!='Cancel':raise ValueError('numeric diagnostic classification changed')
   continue
  if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions']):raise ValueError('incomplete control workflow')
  accepted=command_result(v)=='Nothing'
  if v['success']is not False or v['history_changed']is not accepted or command_result(v)not in ['Nothing','Cancel']:raise ValueError('control Native result changed')
  if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('control Native history changed')
  if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid control geometry')
  for before in v['before']:
   if 'brep'in before and before!=next(x for x in v['after']if x['attributes']['name']==before['attributes']['name']):raise ValueError('control surface parent changed')
  o=output(v)
  if accepted:
   if o is None or len(o['samples'])!=33:raise ValueError('missing control Native stations')
   stations+=33;joins+=o['native_type']=='PolyCurve'
   if not o['guide']and(o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']):raise ValueError('control output attributes changed')
  else:
   if o is not None or normalized(v['before'])!=normalized(v['after'])or v['pending_redo']is None:raise ValueError('control cancellation changed history')
   cancelled+=1
  for c in trace['clicks']:
   a=recipe['actions'][c['action']]
   if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or c.get('shift_key_down')is not bool(a.get('shift',False)):raise ValueError('control public click receipt changed')
   if 'edge_pick_context'in c:prior.prior.prior.nearest(c)
   picks+=1;shift+=c['shift_key_down']
  for phase in ['before','after']:
   for x in v[phase]:
    if 'curve'not in x:continue
    samples=x.get('derivative_samples',[])
    if len(samples)!=3 or any(len(s['derivatives'])!=5 for s in samples):raise ValueError('missing control derivatives')
    derivatives+=3
  if accepted and orders(v)==[4,4]and o['native_type']!='PolyCurve':
   for sample in [o['derivative_samples'][0],o['derivative_samples'][-1]]:
    d=sample['derivatives'][1:];speed=math.sqrt(sum(x*x for x in d[0]));t=d[0];t=[x/speed for x in t];d=[[x/speed**(i+1)for x in row]for i,row in enumerate(d)]
    dot=lambda x,y:sum(a*b for a,b in zip(x,y))
    a=dot(d[1],t);b=dot(d[2],t);c=dot(d[3],t);k=[x-a*y for x,y in zip(d[1],t)];j=[x-3*a*y-b*z for x,y,z in zip(d[2],k,t)]
    prediction=[6*a*x+(12*a*(b+dot(k,k))+9*a**3)*y+c*z for x,y,z in zip(j,k,t)]
    fourth_errors.append(max(abs(x-y)for x,y in zip(prediction,d[3])))
 policy=dict(fourth_response='6a*J + (12a*(b+K.K)+9a^3)*K + c*T',maximum_native_fourth_policy_residual=max(fourth_errors),fourth_endpoint_observations=len(fourth_errors),long_rational_domain=audit)
 if policy['maximum_native_fourth_policy_residual']>1e-6:raise ValueError('fourth control policy changed')
 return workflows,summary,dict(physical_picks=picks,shift_clicks=shift,point_tangent_stations=stations,derivative_stations=derivatives,joined_workflows=joins,cancelled_workflows=cancelled),policy,paths
