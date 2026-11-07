"""Public viewport evidence and transient source-analysis lifetimes."""
import base64,gzip,hashlib,json,re,struct
from . import blend_curve_command as recipes
from .arc_blend_radius_evidence import finite,HERE
from .blend_curve_evidence import command_result,normalized,output
from .blend_curve_high_evidence import captures

BASELINE='5be9fff29cba6e965f05803c410e2ca24d215de1'
FAMILIES=dict(blend_curve_display_pilot=list(recipes.DISPLAY_PILOT),
 blend_curve_display_discovery=list(recipes.DISPLAY_DISCOVERY),
 blend_curve_display_lifecycle=list(recipes.DISPLAY_LIFECYCLE),
 blend_curve_display_verification=list(recipes.DISPLAY_VERIFICATION),
 blend_curve_display_defaults=list(recipes.DISPLAY_DEFAULTS),
 blend_curve_display_control_defaults=list(recipes.DISPLAY_CONTROL_DEFAULTS))
sha=lambda data:hashlib.sha256(data).hexdigest()

def collect(proof):
 rows,summary,paths=captures(proof,FAMILIES)
 for receipt in json.loads((proof/'captures.json').read_bytes()):
  family=receipt['family'];raw=gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes())
  if sha(raw)!=receipt['observation_sha256']or sha((HERE/('fixtures/'+family+'.json')).read_bytes())!=receipt['request_sha256']:raise ValueError('display capture digest changed')
 attempt,=json.loads((proof/'capture-attempts.json').read_bytes())
 if attempt['exit_status']!=130 or attempt['native_observations']!=0 or 'KeyboardInterrupt'not in gzip.decompress((proof/attempt['log']).read_bytes()).decode()or 'Selection Menu'not in gzip.decompress((proof/attempt['inspection_log']).read_bytes()).decode():raise ValueError('display interrupted capture changed')
 request=HERE/('fixtures/'+attempt['family']+'.json');paths.add(request)
 if sha(request.read_bytes())!=attempt['request_sha256']:raise ValueError('display failed request changed')
 for digest in attempt['sources'].values():
  path=HERE/('observations/source_snapshots/'+digest+'.gz');paths.add(path)
  if sha(gzip.decompress(path.read_bytes()))!=digest:raise ValueError('display failed producer changed')
 attempts=json.loads((proof/'validation-attempts.json').read_bytes())
 expected=[('command-setup-group-expansion','completed analysis'),('ui-setup-group-expansion','completed analysis'),('command-continuity-initial-state','curvature=6.223')]
 if [(a['name'],a['status'])for a in attempts[:3]]!=[(name,101)for name,_ in expected]or any(needle not in gzip.decompress((proof/a['log']).read_bytes()).decode()for a,(_,needle)in zip(attempts,expected)):raise ValueError('display initial validation attempts changed')
 pilot=rows.pop('blend_curve_display_pilot/display_prompt')
 if pilot['input_trace'][0]['completed']!=8 or pilot['input_trace'][0]['expected']!=9 or pilot['input_trace'][0]['errors']!=['incomplete owned BlendCrv input']or command_result(pilot)!='Nothing':raise ValueError('display pilot diagnostic changed')
 repeat=rows['blend_curve_display_discovery/display_prompt_repeat']['prescribed_case']
 if pilot['prescribed_case']['actions'][:-1]!=repeat['actions']:raise ValueError('display prompt repeat changed')
 counts=dict(accepted_workflows=0,cancelled_workflows=0,physical_picks=0,point_tangent_stations=0,derivative_stations=0,curvature_stations=0,viewport_snapshots=0,analysis_snapshots=0)
 for family in list(FAMILIES)[1:]:
  continuity=[2,2]
  for name in FAMILIES[family]:
   value=rows[family+'/'+name];recipe=recipes.CASES[name];trace=value['input_trace'][0]
   if value['prescribed_case']!=recipe or not value['returned_idle']or not finite(value):raise ValueError('display recipe or finite geometry changed')
   if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or 'Unknown command'in value['history']:raise ValueError('incomplete display workflow')
   initial=[int(re.search('Continuity_'+str(i+1)+'=G([0-4])',trace['prompts'][0]).group(1))for i in range(2)]
   if initial!=continuity:raise ValueError('display continuity preference sequence changed')
   whole=set();private=False
   for index,action in enumerate(recipe['actions']):
    if action=='SubCrv':private=True
    elif isinstance(action,dict)and'curve'in action:
     if not private:whole.add('guide'+str(action['curve']))
     private=False
    elif isinstance(action,str):
     match=re.fullmatch(r'Continuity_([12])=G([0-4])',action)
     if match and trace['prompts'][index].startswith('Select control point'):continuity[int(match[1])-1]=int(match[2])
   accepted=command_result(value)=='Nothing'
   if command_result(value)not in ['Nothing','Cancel']or value['success']is not False or value['history_changed']is not accepted:raise ValueError('display Native result changed')
   if normalized(value['before'])!=normalized(value['undo'])or normalized(value['after'])!=normalized(value['redo']):raise ValueError('display Native history changed')
   if any(not o['native_valid']for phase in ['before','after','undo','redo']for o in value[phase]):raise ValueError('invalid display Native geometry')
   parents=[o for o in value['after']if o['attributes']['name']]
   if normalized(parents)!=normalized(value['before']):raise ValueError('display modified a parent')
   if accepted:
    obj=output(value)
    if obj is None or len(obj['samples'])!=33 or obj['attributes']['layer']!='Plane output'or obj['attributes']['groups']:raise ValueError('missing display Native output')
    counts['accepted_workflows']+=1;counts['point_tangent_stations']+=33
   else:
    if value['pending_redo']is None:raise ValueError('display cancellation lost redo')
    counts['cancelled_workflows']+=1
   for click in trace['clicks']:
    if not click['motion_acknowledged']or not click['click_acknowledged']or len(click['ray'])!=2:raise ValueError('display public input receipt changed')
    counts['physical_picks']+=1
   initial_graphs={'guide'+str(i)for i in recipe.get('initial_graphs',[])}
   for phase,shots in [('input',trace.get('viewport_snapshots',[])),('completion',value.get('completion_observation',[]))]:
    for shot in shots:
     png=base64.b64decode(shot.get('png_base64',''),validate=True)
     if shot['captured']is not True or not png.startswith(b'\x89PNG\r\n\x1a\n')or len(png)<24 or struct.unpack('>II',png[16:24])!=(shot['width'],shot['height']):raise ValueError('invalid display viewport evidence')
     settings=shot['settings'];expected_settings=recipe.get('graph_settings',dict(scale=100,density=1))
     if settings!=dict(expected_settings,sample_density=2):raise ValueError('display graph settings changed')
     if 'object_graphs'in shot:
      enabled=bool(initial_graphs)or any(a=='DisplayCurvature'for a in recipe['actions'][:shot['action']])if phase=='input'else False
      expected_graphs=initial_graphs|(whole if enabled else set())
      if {o['name']for o in shot['object_graphs']if o['enabled']}!=expected_graphs:raise ValueError('display transient analysis lifetime changed')
      counts['analysis_snapshots']+=1
     counts['viewport_snapshots']+=1
   for phase in ['before','after']:
    for obj in value[phase]:
     if 'curve'not in obj:continue
     derivatives=obj.get('derivative_samples',[])
     if len(derivatives)!=3 or any(len(s['derivatives'])!=5 for s in derivatives):raise ValueError('missing display derivatives')
     counts['derivative_stations']+=3
     samples=obj.get('curvature_samples',[])
     if recipe.get('measure_curvature')and(len(samples)!=33 or any(s['fraction']!=i/32. for i,s in enumerate(samples))):raise ValueError('missing display curvature stations')
     counts['curvature_stations']+=len(samples)
 if len(rows)!=26:raise ValueError('display workflow inventory changed')
 return rows,summary,counts,paths
