"""Direct mesh-face construction, input controls and grouped-history qualification."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
GROUPS=[('',18),('_variants',7),('_controls',5),('_drafts',12),('_draft_extras',6)]
SOURCES=[
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/face_create.rs',
 'crates/viboceros-geometry/src/mesh/append.rs','crates/viboceros-geometry/src/mesh/ngon.rs',
 'crates/viboceros-geometry/src/mesh/closed_polyline.rs','crates/viboceros-geometry/src/mesh/patch.rs',
 'crates/viboceros-geometry/src/mesh/normals.rs','crates/viboceros-geometry/src/mesh/projection.rs',
 'crates/viboceros-geometry/src/mesh/union_find.rs','crates/viboceros-geometry/src/frame.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/face_create.rs',
 'crates/viboceros-command/tests/face_create_native.rs','crates/viboceros-command/tests/support/face_create.rs',
 'src/viewport.rs','src/viewport/edge_picking.rs','src/viewport/edge_picking/tests.rs',
 'src/app.rs','src/app/face_create.rs','src/app/face_create/tests.rs','src/app/mesh_face_prompt.rs',
 'tools/rhino_oracle/generate_face_draft_validity.py','tools/rhino_oracle/fixtures/face_draft_validity.json','tools/rhino_oracle/observations/face_draft_validity.json',
 'tools/rhino_oracle/face_draft_cases.py','tools/rhino_oracle/face_draft_input.py',
 'tools/rhino_oracle/face_create_probe.py','tools/rhino_oracle/face_create_input.py',
 'tools/rhino_oracle/generate_face_create.py','tools/rhino_oracle/report_face_create.py','tools/rhino_oracle/test_face_create.py',
 'tools/rhino_oracle/group_picking.py','tools/rhino_oracle/client.py','tools/rhino_oracle/structural_sdk.py','tools/rhino_oracle/rhino_worker.py',
]+[f'tools/rhino_oracle/{directory}/face_create{suffix}.json'for suffix,_ in GROUPS for directory in ['fixtures','observations']]


def check(row,value,allow_gaps=False):
 qualified=True
 c=row['check'];targets=value['after_script']
 if len(c['meshes'])!=len(targets):raise ValueError('missing mesh comparison')
 for actual,target in zip(c['meshes'],targets):
  for key in ['actual_vertices','native_vertices','actual_faces','native_faces','actual_ngons','native_ngons']:
   if type(actual.get(key))is not int or actual[key]<0:raise ValueError('invalid topology count')
  if (actual['native_vertices'],actual['native_faces'],actual['native_ngons'])!=(len(target['mesh']['vertices']),len(target['mesh']['faces']),len(target['ngons'])):
   raise ValueError('native topology changed')
  same=all(actual['actual_'+k]==actual['native_'+k]for k in ['vertices','faces','ngons'])
  error=actual.get('maximum_bidirectional_sample_error')
  if type(error)not in (int,float)or not math.isfinite(error)or error<0:raise ValueError('invalid mesh-face comparison')
  mesh_qualified=error<=1e-6 and same and actual['counts_match']is True and actual['ngons_match']is True
  if actual['qualified']is not mesh_qualified:raise ValueError('hidden mesh-face comparison gap')
  qualified &= mesh_qualified
 qualified &= c['attributes_and_identity_preserved']is True
 if c['qualified']is not qualified or row['qualified']is not qualified or row['grouped_history']is not True:raise ValueError('hidden source, geometry or history gap')
 if not allow_gaps and not qualified:raise ValueError('unexpected baseline face gap')


def summarize(log):
 groups={};maximum=0.
 for suffix,count in GROUPS:
  q=json.loads((HERE/f'fixtures/face_create{suffix}.json').read_text())
  native=json.loads((HERE/f'observations/face_create{suffix}.json').read_text())
  ids=[op['id']for op in q['operations']]
  if len(ids)!=count or len(set(ids))!=count or [r['id']for r in native['results']]!=ids:raise ValueError('incomplete native face capture')
  stages={}
  for stage in (['draft_command','draft_getter']if suffix.startswith('_draft')else['controls']if suffix=='_controls'else['command','getter']):
   marker=('FACE_'+stage.upper())if stage.startswith('draft_')else'FACE_CREATE_'+stage.upper()
   wanted_ids=[id_ for id_ in ids if not(stage=='draft_command'and id_=='chain_cancel')]
   rows=[json.loads(s)for s in re.findall(marker+r' (\{[^\n]+\})',log)]
   rows=[r for r in rows if r['id']in wanted_ids]
   if [r['id']for r in rows]!=wanted_ids:raise ValueError('missing or duplicate '+marker+' replay')
   values={r['id']:r['value']for r in native['results']}
   for row in rows:
    check(row,values[row['id']],suffix.startswith('_draft'));maximum=max(maximum,max((m['maximum_bidirectional_sample_error']for m in row['check']['meshes']),default=0.))
   stages[stage]=dict(recipes=len(rows),qualified_recipes=sum(row['qualified']for row in rows),unqualified_recipes=[row['id']for row in rows if not row['qualified']],cases=rows)
  groups[suffix.lstrip('_')or'base']=dict(native_engine_version=native['engine_version'],**stages)
 for stage in ['controls']:
  expected=[op['id']for op in json.loads((HERE/'fixtures/face_create_controls.json').read_text())['operations']]
  actual=[json.loads(s)['id']for s in re.findall('FACE_CREATE_CONTROLS'+r' (\{[^\n]+\})',log)]
  if actual!=expected:raise ValueError('extra or missing physical control replay')
 for stage in ['command','getter']:
  expected=[r['id']for suffix,_ in GROUPS[:2]for r in json.loads((HERE/f'observations/face_create{suffix}.json').read_text())['results']]
  actual=[json.loads(s)['id']for s in re.findall('FACE_CREATE_'+stage.upper()+r' (\{[^\n]+\})',log)]
  if actual!=expected:raise ValueError('extra, reordered or missing face replay')
 for stage in ['draft_command','draft_getter']:
  expected=[op['id']for suffix,_ in GROUPS[3:]for op in json.loads((HERE/f'fixtures/face_create{suffix}.json').read_text())['operations']if not(stage=='draft_command'and op['id']=='chain_cancel')]
  found=[json.loads(row)['id']for row in re.findall('FACE_'+stage.upper()+r' (\{[^\n]+\})',log)]
  if found!=expected:raise ValueError('extra, reordered or missing chain replay')
 validity=json.loads((HERE/'observations/face_draft_validity.json').read_text())
 expected=[op['id']for suffix,_ in GROUPS[3:]for op in json.loads((HERE/f'fixtures/face_create{suffix}.json').read_text())['operations']]
 if [row['id']for row in validity['results']]!=expected:raise ValueError('missing native mesh validity controls')
 statuses={row['id']:row['value']['status']for row in validity['results']}
 if any(status not in ('valid','invalid')for status in statuses.values()):raise ValueError('unknown mesh validity result')
 return dict(native_record_validity=statuses,model_epsilon=1e-6,full_native_parity=False,performance_qualified=False,recipes=48,maximum_sample_difference=maximum,groups=groups,
  guarantees=[
   'Twenty-five captured mesh recipes match command and point-getter samples, raw vertex/face counts, triangle/quad kinds and cyclic n-gon boundary winding.',
   'Five physical native input controls compare edge attachment and cancellation through the interactive getter. Eighteen additional physical edge-chain scenes retain every command and viewport-getter result, including unresolved comparisons.',
   'Append keeps source identity, name and layer; output selection and grouped Undo/Redo match each captured result.',
   'Zero-degree welding joins coplanar seams. Welding uses exact coordinate adjacency and preserves unrelated pre-existing unwelded seams.',
   'Draft corners and previews leave the model and Redo unchanged until completion. History or source changes invalidate a stale draft.',
   'Continuous chain walks cache an oriented source boundary loop; reverse walks retain coherent winding. Completed loops can continue onto newly created boundaries. Screen-space picks target current draft boundaries without admitting temporary document objects.',
   'Ordered n-gons reuse the existing constrained polygon triangulator and validated mesh n-gon overlays. InPlane projects later corners onto the first-three-corner plane.',
  ],limits=[
   'SubD output is explicitly unsupported. chain_wrap, edge_collinear_inner and chain_wrap_shift remain unqualified geometry or topology comparisons. Public Rhino IsValidWithLog accepts all eighteen captured draft mesh records.',
   'Quad surface comparisons use the shared shortest-diagonal triangulation on both sides. Native display tessellation of warped quads, raw vertex/face ordering, and stored normal arrays are not certified.',
   'General self-intersecting polygons, nonmanifold append neighborhoods, texture/vertex-color channels and native comparative speed require further qualification.',
   'Construction retains at most 100000 corners, including automatically reused chain corners. Per-face option changes retain prior completed faces; the usual constant-angle path appends and welds once.',
   'Legacy A-C diagonal rejection is recorded for native quads. Comparisons use nondegenerate triangles from the shortest-diagonal split; original native vertex, face and n-gon counts remain the topology targets.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
 (ROOT/'docs/face-create-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
