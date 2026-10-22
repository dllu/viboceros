"""SubDBox command, viewport getter, native diagnostics and grouped history."""
from .qualification_sources import with_shared_sources
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
 'crates/viboceros-command/src/subd_plane.rs','crates/viboceros-command/tests/support/subd_plane.rs',
 'crates/viboceros-opennurbs/native/subd.cpp','crates/viboceros-opennurbs/native/subd.h','crates/viboceros-opennurbs/native/subd_codec.h','crates/viboceros-opennurbs/src/subd.rs','crates/viboceros-opennurbs/src/lib.rs',
 'crates/viboceros-geometry/src/subd.rs','crates/viboceros-geometry/src/subd/primitives.rs','crates/viboceros-geometry/src/frame.rs','crates/viboceros-geometry/src/point.rs','crates/viboceros-geometry/src/vector.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/subd_box.rs','crates/viboceros-command/src/rectangle_input.rs','crates/viboceros-command/src/curve_reference.rs',
 'crates/viboceros-command/tests/subd_box_native.rs','crates/viboceros-command/tests/support/subd_box.rs','crates/viboceros-command/tests/support/subd.rs',
 'src/app.rs','src/app/subd_primitives.rs','src/app/subd_primitives/tests.rs','src/app/rectangle.rs','src/app/point_input.rs',
 'tools/rhino_oracle/subd_box_probe.py','tools/rhino_oracle/generate_subd_box.py','tools/rhino_oracle/structural_sdk.py','tools/rhino_oracle/rhino_worker.py',
 'tools/rhino_oracle/fixtures/subd_box.json','tools/rhino_oracle/observations/subd_box.json','tools/rhino_oracle/report_subd_box.py','tools/rhino_oracle/test_subd_box.py',
]
SOURCES = with_shared_sources(SOURCES)
def error(value):
 if type(value)not in(int,float)or not math.isfinite(value)or not 0<=value<=1e-6:raise ValueError('invalid or excessive SubDBox error')
 return value

def summarize(text):
 request=json.loads((HERE/'fixtures/subd_box.json').read_text());native=json.loads((HERE/'observations/subd_box.json').read_text());expected={r['id']:r['value']for r in native['results']}
 if len(native['results'])!=23 or len(expected)!=23 or list(expected)!=[o['id']for o in request['operations']]:raise ValueError('missing native box observations')
 unsupported=[key for key,v in expected.items()if v['input'].get('unsupported')]
 if len(unsupported)!=6 or any('Unknown command:'not in expected[key]['history']for key in unsupported):raise ValueError('missing native option diagnostics')
 supported={k:v for k,v in expected.items()if k not in unsupported}
 for value in supported.values():
  if 'Unknown command:'in value['history']or value['after_script']!=value['redo']or value['undo']!=value['before']:raise ValueError('invalid native box history')
  if value['success']is value['input'].get('cancel',False):raise ValueError('unexpected native command result')
 maximum_control=maximum_surface=0.;stages={}
 for stage in ['COMMAND','GETTER']:
  rows=[json.loads(s)for s in re.findall(r'^SUBD_BOX_'+stage+r' (.+)$',text,re.M)]
  ids=[k for k,v in supported.items()if stage=='GETTER'or not v['input'].get('cancel')]
  if [r['id']for r in rows]!=ids:raise ValueError('missing, duplicate or reordered box '+stage)
  for row in rows:
   check=row['check'];targets=[o['subd']for o in supported[row['id']]['after_script']if 'subd'in o]
   if row.get('grouped_history')is not True or check.get('qualified')is not True or check.get('attributes_selection_and_source')is not True or len(check['objects'])!=len(targets):raise ValueError('unqualified box attributes/history/outputs')
   for actual,target in zip(check['objects'],targets):
    if actual.get('qualified')is not True or target.get('valid')is not True or actual.get('surface_sampled')is not True:raise ValueError('unqualified box geometry')
    for key,values in [('vertices',target['points']),('edges',target['edges']),('faces',target['faces'])]:
     if type(actual.get(key))is not int or actual[key]!=len(values):raise ValueError('box topology differs')
    maximum_control=max(maximum_control,error(actual.get('maximum_control_or_limit_point_error')));maximum_surface=max(maximum_surface,error(actual.get('maximum_sampled_surface_error')))
  stages[stage.lower()]=rows
 return dict(model_epsilon=1e-6,captured_recipes=23,supported_recipes=17,command_cases=16,getter_cases=17,native_engine_version=native['engine_version'],
  full_native_parity=False,performance_qualified=False,maximum_control_or_limit_point_error=maximum_control,maximum_sampled_surface_error=maximum_surface,
  native_unsupported_options=unsupported,stages=stages,
  guarantees=[
   'Smooth native SubD boxes use OpenNURBS CreateSubDBox, preserving its original topology and face packing in the complete SubD archive.',
   'Native default two-by-two-by-two divisions, individual counts, a uniform leading count, zero at the CPlane origin, numeric rectangle dimensions, height points and Enter for default Y-width height match the captured results.',
   'Corner, reversed-corner, negative-height, Center, spatial 3Point, Vertical and AroundCurve constructions match native topology, tags, control points, exact vertex limit stations and density-two surface samples at model epsilon.',
   'Command and viewport getters preserve current-layer output, source curve geometry and selection, and grouped Undo/Redo. Interactive options, cancellation and previews leave model geometry/history staged; a changed history or tolerance invalidates the draft.',
  ],limits=[
   'The installed Rhino build rejects Diagonal, Cube and EdgeMidpoint for SubDBox, although its online help lists them. Six raw diagnostic captures retain those unknown-command results, including any fallback geometry; they are excluded from supported geometry comparisons. Viboceros rejects these construction options.',
   'AroundCurve comparisons cover two straight guides. General curved guides and surface-edge frame parity need further native qualification.',
   'Preview outlines show the construction cage; smooth limit surfaces appear on completion. Facet sampling is not a continuous Hausdorff certificate.',
   'Counts must be positive; at most 100000 control vertices are allowed. Native comparative speed, arbitrary input clocks and every mouse trajectory remain unqualified.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
 (ROOT/'docs/subd-box-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
