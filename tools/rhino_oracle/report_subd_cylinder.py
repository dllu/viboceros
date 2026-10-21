"""SubDCylinder caps, surface radius, spatial frames and grouped history."""
import argparse,hashlib,json,re
from pathlib import Path
from .report_subd_box import ROOT,HERE,SOURCES as BASE_SOURCES,error
SOURCES=list(dict.fromkeys(BASE_SOURCES+[
 'crates/viboceros-command/src/subd_axial.rs','crates/viboceros-command/src/subd_axial/input.rs','crates/viboceros-command/src/subd_axial/input/axis.rs','crates/viboceros-command/src/subd_axial/input/options.rs','crates/viboceros-command/src/subd_axial/input/references.rs','crates/viboceros-command/src/subd_axial/arguments.rs','crates/viboceros-command/src/subd_axial/adapter.rs','crates/viboceros-command/src/subd_axial/end_circle.rs',
 'crates/viboceros-geometry/src/subd/cylinder.rs','crates/viboceros-command/src/circle_input.rs','crates/viboceros-geometry/src/circle_curve.rs','src/app/subd_axial.rs','src/app/subd_axial/tests.rs',
 'crates/viboceros-command/src/subd_cylinder.rs','crates/viboceros-command/tests/subd_cylinder_native.rs','crates/viboceros-command/tests/support/subd_cylinder.rs',
 'tools/rhino_oracle/subd_cylinder_probe.py','tools/rhino_oracle/generate_subd_cylinder.py','tools/rhino_oracle/report_subd_cylinder.py','tools/rhino_oracle/test_subd_cylinder.py',
 'tools/rhino_oracle/fixtures/subd_cylinder.json','tools/rhino_oracle/observations/subd_cylinder.json',
]))
def summarize(text):
 q=json.loads((HERE/'fixtures/subd_cylinder.json').read_text());native=json.loads((HERE/'observations/subd_cylinder.json').read_text());expected={r['id']:r['value']for r in native['results']}
 if len(native['results'])!=28 or len(expected)!=28 or list(expected)!=[o['id']for o in q['operations']]:raise ValueError('missing native cylinder observations')
 diagnostics=[key for key,v in expected.items()if v['input'].get('unsupported')]
 if len(diagnostics)!=3 or any('Unknown command:'not in expected[key]['history']for key in diagnostics):raise ValueError('missing native cylinder diagnostics')
 supported={k:v for k,v in expected.items()if k not in diagnostics}
 for v in supported.values():
  if 'Unknown command:'in v['history']or v['after_script']!=v['redo']or v['before']!=v['undo']:raise ValueError('invalid native cylinder history')
  if v['success']is v['input'].get('cancel',False):raise ValueError('unexpected native cylinder result')
 maximum_control=maximum_surface=0.;stages={}
 for stage in ['COMMAND','GETTER']:
  rows=[json.loads(s)for s in re.findall(r'^SUBD_CYLINDER_'+stage+r' (.+)$',text,re.M)];ids=[key for key,v in supported.items()if stage=='GETTER'or not v['input'].get('cancel')]
  if [r['id']for r in rows]!=ids:raise ValueError('missing, duplicate or reordered cylinder '+stage)
  for row in rows:
   actual=row['check'];targets=[o['subd']for o in supported[row['id']]['after_script']if 'subd'in o]
   if row.get('grouped_history')is not True or actual.get('qualified')is not True or actual.get('attributes_selection_and_source')is not True or len(actual['objects'])!=len(targets):raise ValueError('unqualified cylinder attributes, history or source')
   for check,target in zip(actual['objects'],targets):
    if check.get('qualified')is not True or check.get('surface_sampled')is not True or target.get('valid')is not True:raise ValueError('unqualified cylinder geometry')
    for key,values in [('vertices',target['points']),('edges',target['edges']),('faces',target['faces'])]:
     if type(check.get(key))is not int or check[key]!=len(values):raise ValueError('cylinder topology differs')
    maximum_control=max(maximum_control,error(check.get('maximum_control_or_limit_point_error')));maximum_surface=max(maximum_surface,error(check.get('maximum_sampled_surface_error')))
  stages[stage.lower()]=rows
 return dict(model_epsilon=1e-6,captured_recipes=28,supported_recipes=25,command_cases=23,getter_cases=25,native_engine_version=native['engine_version'],full_native_parity=False,performance_qualified=False,
  maximum_control_or_limit_point_error=maximum_control,maximum_sampled_surface_error=maximum_surface,native_diagnostics=diagnostics,stages=stages,
  guarantees=[
   'SubDCylinder uses the public OpenNURBS CreateCylinder constructor with surface-radius placement and smooth cap edges. The complete SubD archive retains native component data.',
   'Native defaults are eight around faces and four vertical faces, solid triangle caps. Open, triangle and quad caps, three and four around faces, and the odd quad-to-triangle fallback match captured topology.',
   'Radius points, scalar radius, Diameter, Circumference, Area, 2Point, spatial 3Point, tilted construction planes, signed height, end points, DirectionConstraint and BothSides match captured control points, tags, vertex limit stations and density-two surface samples.',
   'Free-axis endpoint picks rotate the base circle perpendicular to the axis, with native seam orientation also checked on odd around-face counts. Changing DirectionConstraint resets BothSides.',
   'Command and interactive getter comparisons preserve current-layer output, object attributes, selection and one grouped Undo/Redo. Previews and cancellation leave model geometry and Redo untouched; history or tolerance changes invalidate a draft.',
  ],limits=[
   'Tangent and FitPoints have a separate qualification report, docs/subd-cylinder-references-qualification.json. Radius/height defaults and cursor-driven numeric free-axis input have regression tests but need separate native sequence captures; arbitrary input clocks and every mouse trajectory remain unqualified.',
   'Three raw diagnostics retain rejected VerticalFaces=1, Vertical and Orientation tokens and their fallback macro effects. The cylinder has no Circle Vertical/Orientation base mode in the captured Rhino build; Vertical resolves to VerticalFaces. Diagnostics are excluded from supported comparisons.',
   'At least three around faces and two vertical faces are required by the command. At most 100000 control vertices and faces are allowed.',
   'The preview shows construction circles and the axis; the smooth surface appears on completion. Sampled facets carry no continuous Hausdorff certificate. Comparative speed and native texture/material parity remain unqualified.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args();(ROOT/'docs/subd-cylinder-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
