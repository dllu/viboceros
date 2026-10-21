"""Direct mesh-face construction, input controls and grouped-history qualification."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
GROUPS=[('',18),('_variants',7),('_controls',5)]
SOURCES=[
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/face_create.rs',
 'crates/viboceros-geometry/src/mesh/append.rs','crates/viboceros-geometry/src/mesh/ngon.rs',
 'crates/viboceros-geometry/src/mesh/closed_polyline.rs','crates/viboceros-geometry/src/mesh/patch.rs',
 'crates/viboceros-geometry/src/mesh/normals.rs','crates/viboceros-geometry/src/mesh/projection.rs',
 'crates/viboceros-geometry/src/mesh/union_find.rs','crates/viboceros-geometry/src/frame.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/face_create.rs',
 'crates/viboceros-command/tests/face_create_native.rs','crates/viboceros-command/tests/support/face_create.rs',
 'src/app.rs','src/app/face_create.rs','src/app/face_create/tests.rs','src/app/mesh_face_prompt.rs',
 'tools/rhino_oracle/face_create_probe.py','tools/rhino_oracle/face_create_input.py',
 'tools/rhino_oracle/generate_face_create.py','tools/rhino_oracle/report_face_create.py','tools/rhino_oracle/test_face_create.py',
 'tools/rhino_oracle/group_picking.py','tools/rhino_oracle/client.py','tools/rhino_oracle/structural_sdk.py','tools/rhino_oracle/rhino_worker.py',
]+[f'tools/rhino_oracle/{directory}/face_create{suffix}.json'for suffix,_ in GROUPS for directory in ['fixtures','observations']]


def check(row,value):
 c=row['check'];targets=value['after_script']
 if len(c['meshes'])!=len(targets):raise ValueError('missing mesh comparison')
 for actual,target in zip(c['meshes'],targets):
  for key in ['actual_vertices','native_vertices','actual_faces','native_faces','actual_ngons','native_ngons']:
   if type(actual.get(key))is not int or actual[key]<0:raise ValueError('invalid topology count')
  if (actual['native_vertices'],actual['native_faces'],actual['native_ngons'])!=(len(target['mesh']['vertices']),len(target['mesh']['faces']),len(target['ngons'])):
   raise ValueError('native topology changed')
  same=all(actual['actual_'+k]==actual['native_'+k]for k in ['vertices','faces','ngons'])
  error=actual.get('maximum_bidirectional_sample_error')
  if type(error)not in (int,float)or not math.isfinite(error)or not 0<=error<=1e-6:raise ValueError('invalid mesh-face comparison')
  if not same or actual['counts_match']is not True or actual['ngons_match']is not True or actual['qualified']is not True:
   raise ValueError('mesh-face topology or boundary gap hidden')
 if any(flag is not True for flag in [c['qualified'],row['qualified'],c['attributes_and_identity_preserved'],row['grouped_history']]):raise ValueError('source or history comparison omitted')


def summarize(log):
 groups={};maximum=0.
 for suffix,count in GROUPS:
  q=json.loads((HERE/f'fixtures/face_create{suffix}.json').read_text())
  native=json.loads((HERE/f'observations/face_create{suffix}.json').read_text())
  ids=[op['id']for op in q['operations']]
  if len(ids)!=count or len(set(ids))!=count or [r['id']for r in native['results']]!=ids:raise ValueError('incomplete native face capture')
  stages={}
  for stage in (['controls']if suffix=='_controls'else['command','getter']):
   marker='FACE_CREATE_'+stage.upper()
   rows=[json.loads(s)for s in re.findall(marker+r' (\{[^\n]+\})',log)]
   wanted=ids if suffix=='_controls'else [r['id']for r in rows if r['id']in ids]
   if suffix!='_controls':rows=[r for r in rows if r['id']in ids]
   if [r['id']for r in rows]!=ids or wanted!=ids:raise ValueError('missing or duplicate '+marker+' replay')
   values={r['id']:r['value']for r in native['results']}
   for row in rows:
    check(row,values[row['id']]);maximum=max(maximum,max((m['maximum_bidirectional_sample_error']for m in row['check']['meshes']),default=0.))
   stages[stage]=dict(recipes=len(rows),qualified_recipes=len(rows),cases=rows)
  groups[suffix.lstrip('_')or'base']=dict(native_engine_version=native['engine_version'],**stages)
 for stage in ['command','getter']:
  expected=[r['id']for suffix,_ in GROUPS[:2]for r in json.loads((HERE/f'observations/face_create{suffix}.json').read_text())['results']]
  actual=[json.loads(s)['id']for s in re.findall('FACE_CREATE_'+stage.upper()+r' (\{[^\n]+\})',log)]
  if actual!=expected:raise ValueError('extra, reordered or missing face replay')
 return dict(model_epsilon=1e-6,full_native_parity=False,performance_qualified=False,recipes=30,maximum_sample_difference=maximum,groups=groups,
  guarantees=[
   'Twenty-five captured mesh recipes match command and point-getter samples, raw vertex/face counts, triangle/quad kinds and cyclic n-gon boundary winding.',
   'Five additional physical native input controls compare edge attachment and cancellation through the interactive getter.',
   'Append keeps source identity, name and layer; output selection and grouped Undo/Redo match each captured result.',
   'Zero-degree welding joins coplanar seams. Welding uses exact coordinate adjacency and preserves unrelated pre-existing unwelded seams.',
   'Draft corners and previews leave the model and Redo unchanged until completion. History or source changes invalidate a stale draft.',
   'Ordered n-gons reuse the existing constrained polygon triangulator and validated mesh n-gon overlays. InPlane projects later corners onto the first-three-corner plane.',
  ],limits=[
   'SubD output, Chain=Yes and FromEdge with MultipleFaces are explicitly unsupported.',
   'Quad surface comparisons use the shared shortest-diagonal triangulation on both sides. Native display tessellation of warped quads, raw vertex/face ordering, and stored normal arrays are not certified.',
   'General self-intersecting polygons, nonmanifold append neighborhoods, texture/vertex-color channels and native comparative speed require further qualification.',
   'Construction accepts at most 100000 corners. Per-face option changes retain prior completed faces; the usual constant-angle path appends and welds once.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
 (ROOT/'docs/face-create-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
