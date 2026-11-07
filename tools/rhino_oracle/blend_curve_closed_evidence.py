"""Whole closed sources retain their parents while open inputs may trim/join."""
import gzip,hashlib,json,math
from . import blend_curve_command as recipes,blend_curve_controls_evidence as prior
from .blend_curve_high_evidence import captures as read_captures
from .blend_curve_evidence import command_result,normalized,output
from .arc_blend_radius_evidence import finite,HERE
from .check_blend_curve_closed import ROOT
BASELINE='e072d5d3b1c0ccab4db815904799fbb9014bd951'
FAMILIES=dict(blend_curve_closed_single=list(recipes.CLOSED_SINGLE),blend_curve_closed_discovery=list(recipes.CLOSED_DISCOVERY),blend_curve_closed_verification=list(recipes.CLOSED_VERIFICATION),blend_curve_closed_holdouts=list(recipes.CLOSED_HOLDOUTS),blend_curve_closed_ownership=list(recipes.CLOSED_OWNERSHIP))
FAMILIES['blend_curve_closed_self_flips']=list(recipes.CLOSED_SELF_FLIPS)
sha=lambda b:hashlib.sha256(b).hexdigest()
def captures(proof):
 rows,summary,paths=read_captures(proof,FAMILIES)
 for receipt in json.loads((proof/'captures.json').read_bytes()):
  family=receipt['family'];raw=gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes())
  if sha(raw)!=receipt['observation_sha256']or sha((HERE/('fixtures/'+family+'.json')).read_bytes())!=receipt['request_sha256']:raise ValueError('closed capture digest changed')
 return rows,summary,paths
def closed(obj):
 samples=obj.get('derivative_samples',[])
 return bool(samples)and math.dist(samples[0]['derivatives'][0],samples[-1]['derivatives'][0])<1e-12
def collect(proof):
 rows,summary,paths=captures(proof)
 *_,inherited=prior.collect(ROOT/'docs/blend-curve-controls-proof');paths.update(inherited);paths.add(ROOT/'docs/blend-curve-controls-validation.json')
 attempts=json.loads((proof/'capture-attempts.json').read_bytes())
 if len(attempts)!=2:raise ValueError('closed failed-capture inventory changed')
 for attempt,needle in zip(attempts,['Rhino process exited before publishing a response','multiple values for keyword argument']):
  if attempt['exit_status']!=1 or attempt['native_observations']!=0 or needle not in gzip.decompress((proof/attempt['log']).read_bytes()).decode():raise ValueError('closed failed-capture receipt changed')
  for digest in attempt['sources'].values():
   p=HERE/('observations/source_snapshots/'+digest+'.gz')
   if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('closed failed producer changed')
   paths.add(p)
  if 'request_sha256'in attempt:
   p=HERE/('fixtures/'+attempt['family']+'.json')
   if sha(p.read_bytes())!=attempt['request_sha256']:raise ValueError('closed failed request changed')
   paths.add(p)
 for name,recipe in recipes.CLOSED_PILOT.items():
  if rows['blend_curve_closed_verification/verified_'+name]['prescribed_case']!=recipe:raise ValueError('closed pilot repeat changed')
 validation=json.loads((proof/'validation-attempts.json').read_bytes())
 expected=[('periodic-command-initial','closed_periodic_g3'),('periodic-ui-initial','closed_periodic_g3'),('canonical-input-initial','closed_periodic_g3'),('archive-padding-diagnostic','archive=true name=Some("guide0")'),('ui-log-access-initial','VecDeque')]
 expected.append(('evidence-inventory-initial','derivative_stations'))
 statuses=[101]*5+[1]
 if [(r['name'],r['status'])for r in validation]!=[(n,s)for (n,_),s in zip(expected,statuses)]or any(needle not in gzip.decompress((proof/r['log']).read_bytes()).decode()for r,(_,needle)in zip(validation,expected)):raise ValueError('closed validation attempt changed')
 counts=dict(physical_picks=0,point_tangent_stations=0,derivative_stations=0,joined_workflows=0,cancelled_workflows=0,closed_parent_checks=0,rejected_options=0,rejection_workflows=0)
 for key,v in rows.items():
  recipe=recipes.CASES[key.split('/')[1]];trace=v['input_trace'][0]
  if v['prescribed_case']!=recipe or not v['returned_idle']or not finite(v):raise ValueError('closed recipe or finite geometry changed')
  if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions']):raise ValueError('incomplete closed workflow')
  accepted=command_result(v)=='Nothing'
  if v['success']is not False or v['history_changed']is not accepted or command_result(v)not in ['Nothing','Cancel']:raise ValueError('closed Native result changed')
  if normalized(v['before'])!=normalized(v['undo'])or normalized(v['after'])!=normalized(v['redo']):raise ValueError('closed Native history changed')
  rejected=[line.removeprefix('Unknown command: _')for line in v['history'].splitlines()if line.startswith('Unknown command: _')]
  if any(word not in ['Trim=No','Trim=Yes','Join=No','Join=Yes']for word in rejected):raise ValueError('unexpected closed rejected option')
  for word in rejected:
   index=recipe['actions'].index(word);key=word.split('=')[0]+'='
   if key in trace['prompts'][index]:raise ValueError('rejected closed option was offered')
  counts['rejected_options']+=len(rejected);counts['rejection_workflows']+=bool(rejected)
  if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in v[phase]):raise ValueError('invalid closed Native geometry')
  for before in v['before']:
   if closed(before):
    after=next(x for x in v['after']if x['attributes']['name']==before['attributes']['name'])
    if normalized([before])!=normalized([after]):raise ValueError('closed source parent changed')
    counts['closed_parent_checks']+=1
  o=output(v)
  if accepted:
   if o is None or len(o['samples'])!=33:raise ValueError('missing closed Native stations')
   if o['guide']:
    parent=next(x for x in v['before']if x['attributes']['name']==o['attributes']['name'])
    if closed(parent)or o['attributes']!=parent['attributes']:raise ValueError('closed join ownership changed')
   elif o['attributes']['name']or o['attributes']['layer']!='Plane output'or o['attributes']['groups']:raise ValueError('closed output attributes changed')
   counts['point_tangent_stations']+=33;counts['joined_workflows']+=o['native_type']=='PolyCurve'
  else:
   if o is not None or normalized(v['before'])!=normalized(v['after'])or v['pending_redo']is None:raise ValueError('closed cancellation changed history')
   counts['cancelled_workflows']+=1
  for c in trace['clicks']:
   a=recipe['actions'][c['action']]
   if not c['motion_acknowledged']or not c['click_acknowledged']or len(c['ray'])!=2 or c['shift_key_down']is not bool(a.get('shift',False)):raise ValueError('closed public input receipt changed')
   if a.get('curve')is not None:
    context=c.get('pick_context',{});references=context.get('references',[])
    if not context.get('success')or len(references)!=1 or references[0]['selection_method']!=1:raise ValueError('closed public curve pick missing')
   counts['physical_picks']+=1
  for phase in ['before','after']:
   for x in v[phase]:
    if 'curve'not in x:continue
    samples=x.get('derivative_samples',[])
    if len(samples)!=3 or any(len(s['derivatives'])!=5 for s in samples):raise ValueError('missing closed derivatives')
    counts['derivative_stations']+=3
 for name in ['upper','lower','far']:
  a=output(rows['blend_curve_closed_discovery/closed_discover_curveend_'+name]);b=output(rows['blend_curve_closed_discovery/closed_discover_pickpoint_'+name])
  if a['curve']['control_points'][0]!=b['curve']['control_points'][0]:raise ValueError('closed contacts changed with BlendStart')
 for name in ['point','two','self']:
  a=output(rows['blend_curve_closed_ownership/closed_ownership_'+name]);b=output(rows['blend_curve_closed_ownership/closed_ownership_'+name+'_join'])
  if a['native_type']!='NurbsCurve'or b['native_type']!='NurbsCurve'or a['curve']!=b['curve']:raise ValueError('closed-only rejected options changed geometry')
 for name in ['first','second']:
  o=output(rows['blend_curve_closed_self_flips/closed_self_flip_'+name]);samples=[o['derivative_samples'][0],o['derivative_samples'][-1]]
  signs=[]
  for sample in samples:
   p,d=sample['derivatives'][:2];t=[-p[1],p[0],0.];signs.append(sum(x*y for x,y in zip(t,d))>0)
  if signs!=([True,True]if name=='first'else[False,False]):raise ValueError('closed self-reference flip policy changed')
 return rows,summary,counts,paths
