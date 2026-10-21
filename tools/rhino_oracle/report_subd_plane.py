"""SubDPlane counts, rectangle modes, rounded boundaries and native diagnostics."""
import argparse,hashlib,json,re
from pathlib import Path
from .report_subd_box import ROOT,HERE,SOURCES as BASE_SOURCES,error
SOURCES=list(dict.fromkeys(BASE_SOURCES+[
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/face_records.rs',
 'crates/viboceros-command/src/subd_plane.rs','crates/viboceros-command/tests/subd_plane_native.rs','crates/viboceros-command/tests/support/subd_plane.rs',
 'tools/rhino_oracle/subd_plane_probe.py','tools/rhino_oracle/generate_subd_plane.py','tools/rhino_oracle/report_subd_plane.py','tools/rhino_oracle/test_subd_plane.py',
 'tools/rhino_oracle/fixtures/subd_plane.json','tools/rhino_oracle/observations/subd_plane.json',
]))
def summarize(text):
 q=json.loads((HERE/'fixtures/subd_plane.json').read_text());native=json.loads((HERE/'observations/subd_plane.json').read_text());expected={r['id']:r['value']for r in native['results']}
 if len(native['results'])!=19 or len(expected)!=19 or list(expected)!=[o['id']for o in q['operations']]:raise ValueError('missing native plane observations')
 diagnostics=[key for key,v in expected.items()if v['input'].get('unsupported')]
 if len(diagnostics)!=3 or any('Unknown command:'not in expected[key]['history']for key in diagnostics):raise ValueError('missing native plane diagnostics')
 supported={k:v for k,v in expected.items()if k not in diagnostics}
 for v in supported.values():
  if 'Unknown command:'in v['history']or v['after_script']!=v['redo']or v['before']!=v['undo']:raise ValueError('invalid native plane history')
  if v['success']is v['input'].get('cancel',False):raise ValueError('unexpected native plane result')
 maximum_control=maximum_surface=0.;stages={}
 for stage in ['COMMAND','GETTER']:
  rows=[json.loads(s)for s in re.findall(r'^SUBD_PLANE_'+stage+r' (.+)$',text,re.M)];ids=[key for key,v in supported.items()if stage=='GETTER'or not v['input'].get('cancel')]
  if [r['id']for r in rows]!=ids:raise ValueError('missing, duplicate or reordered plane '+stage)
  for row in rows:
   actual=row['check'];targets=[o['subd']for o in supported[row['id']]['after_script']if 'subd'in o]
   if row.get('grouped_history')is not True or actual.get('qualified')is not True or actual.get('attributes_selection_and_source')is not True or len(actual['objects'])!=len(targets):raise ValueError('unqualified plane attributes, history or source')
   for check,target in zip(actual['objects'],targets):
    if check.get('qualified')is not True or check.get('surface_sampled')is not True or target.get('valid')is not True:raise ValueError('unqualified plane geometry')
    for key,values in [('vertices',target['points']),('edges',target['edges']),('faces',target['faces'])]:
     if type(check.get(key))is not int or check[key]!=len(values):raise ValueError('plane topology differs')
    maximum_control=max(maximum_control,error(check.get('maximum_control_or_limit_point_error')));maximum_surface=max(maximum_surface,error(check.get('maximum_sampled_surface_error')))
  stages[stage.lower()]=rows
 return dict(model_epsilon=1e-6,captured_recipes=19,supported_recipes=16,command_cases=15,getter_cases=16,native_engine_version=native['engine_version'],full_native_parity=False,performance_qualified=False,
  maximum_control_or_limit_point_error=maximum_control,maximum_sampled_surface_error=maximum_surface,native_diagnostics=diagnostics,stages=stages,
  guarantees=[
   'SubDPlane reuses the existing rectangular grid sampler and public OpenNURBS polygon construction. Interior tags are smooth and boundary tags are crease, including rounded corner limit stations.',
   'Native default two-by-two divisions, independent and uniform counts, zero at the CPlane origin, numeric dimensions, reversed picks, Center, spatial 3Point, late 3Point, Vertical and AroundCurve match captured geometry.',
   'The command completes when the rectangle is finished. Counts are set at the initial prompt; the shared edit-free rectangle getter supplies points, dimensions and curve frames.',
   'Command and interactive getters compare topology, winding, tags, vertex limit stations and density-two sampled limit surfaces at model epsilon, with current-layer output, source preservation, selection and grouped Undo/Redo.',
   'Box and plane inputs share one application session. Rectangle scripts share curve capture and value admission across Rectangle, SubDBox and SubDPlane.',
  ],limits=[
   'The installed Rhino build rejects EdgeMidpoint for SubDPlane. Two other raw diagnostics retain count/uniform/cancel tokens sent after the native command already completed; those fallback macro effects are excluded from supported geometry comparisons.',
   'AroundCurve coverage uses a straight guide. General curved guide and surface-edge frame parity, every input clock and mouse trajectory need further native qualification.',
   'The preview shows the construction rectangle. The rounded limit surface appears on completion; facet sampling is not a continuous Hausdorff certificate.',
   'At most 100000 control vertices are allowed. Comparative speed, native face-packing metadata, texture and material parity remain unqualified.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args();(ROOT/'docs/subd-plane-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
