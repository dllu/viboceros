"""Complete public mesh polyline replays with explicit compatibility gaps."""
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
 'crates/viboceros-command/src/lib.rs','crates/viboceros-geometry/src/error.rs','crates/viboceros-geometry/src/mesh/normals.rs','crates/viboceros-geometry/src/mesh/mass_triangles.rs','crates/viboceros-geometry/src/mesh/closest.rs',
 'crates/viboceros-geometry/src/mesh/projection.rs','crates/viboceros-geometry/src/mesh/projection/polyline_path.rs',
 'crates/viboceros-geometry/src/mesh/projection/polyline_path/boundary.rs',
 'crates/viboceros-geometry/src/mesh/projection/tests.rs','crates/viboceros-geometry/src/mesh/section.rs','crates/viboceros-geometry/src/mesh/section/tests.rs',
 'crates/viboceros-command/src/polyline_on_mesh.rs','crates/viboceros-command/tests/polyline_on_mesh_native.rs','crates/viboceros-command/tests/support/mesh_polyline.rs',
 'src/app.rs','src/app/polyline_on_mesh.rs','src/app/tests/polyline_on_mesh.rs','src/app/mesh_face_prompt.rs','src/viewport.rs','src/viewport/normal_point.rs',
 'tools/rhino_oracle/polyline_on_mesh_probe.py','tools/rhino_oracle/generate_polyline_on_mesh.py','tools/rhino_oracle/test_polyline_on_mesh.py','tools/rhino_oracle/report_mesh_polyline.py',
 'tools/rhino_oracle/fixtures/polyline_on_mesh.json','tools/rhino_oracle/observations/polyline_on_mesh.json',
 'tools/rhino_oracle/generate_mesh_polyline_boundaries.py','tools/rhino_oracle/generate_mesh_polyline_ties.py',
 'tools/rhino_oracle/fixtures/mesh_polyline_boundaries.json','tools/rhino_oracle/observations/mesh_polyline_boundaries.json',
 'tools/rhino_oracle/fixtures/mesh_polyline_ties.json','tools/rhino_oracle/observations/mesh_polyline_ties.json',
]
def summarize(log):
 def rows(prefix,fixture):
  q=json.loads((HERE/('fixtures/'+fixture+'.json')).read_text());ids=[o['id']for o in q['operations']]
  found=[json.loads(s)for s in re.findall(prefix+r' (\{[^\n]+\})',log)]
  if [r['id']for r in found]!=ids:raise ValueError('incomplete or duplicate '+prefix+' replay')
  for r in found:
   if r['id']=='off_mesh':
    if r['qualified']:raise ValueError('known native gap hidden')
   elif prefix=='MESH_POLYLINE_COMMAND'and r['id']=='cancel':
    if r['qualified']or not r['interactive_only']:raise ValueError('cancellation belongs to the interactive getter')
   else:
    e=r['maximum_bidirectional_sample_error']
    if not math.isfinite(e)or not 0<=e<1e-6 or r['qualified']is not True:raise ValueError('invalid mesh polyline comparison')
  return dict(recipes=len(found),qualified_recipes=sum(r['qualified']for r in found),unqualified_recipes=[r['id']for r in found if not r['qualified']],cases=found)
 command=rows('MESH_POLYLINE_COMMAND','polyline_on_mesh');getter=rows('MESH_POLYLINE_GETTER','polyline_on_mesh')
 boundary_command=rows('MESH_BOUNDARY_COMMAND','mesh_polyline_boundaries');boundary_getter=rows('MESH_BOUNDARY_GETTER','mesh_polyline_boundaries')
 tie_command=rows('MESH_BOUNDARY_TIE_COMMAND','mesh_polyline_ties');tie_getter=rows('MESH_BOUNDARY_TIE_GETTER','mesh_polyline_ties')
 groups=[command,getter,boundary_command,boundary_getter,tie_command,tie_getter]
 return dict(model_epsilon=1e-6,full_native_parity=False,command_registered=True,command_cases=command,getter_cases=getter,
  boundary_command_cases=boundary_command,boundary_getter_cases=boundary_getter,tie_command_cases=tie_command,tie_getter_cases=tie_getter,
  native_engine_version=json.loads((HERE/'observations/polyline_on_mesh.json').read_text())['engine_version'],
  maximum_sample_difference=max(r.get('maximum_bidirectional_sample_error',0)for group in groups for r in group['cases']),
  guarantees=['Immutable mesh parents, current output layer, selection behavior and grouped Undo/Redo in the supported native scenes.','Directed starting-face plane sections cross folds; one original polygon face uses a direct chord, including warped quads.','Exact coincident unwelded vertices share section topology without editing the mesh.','Interrupted sections follow naked boundaries; original boundary corners can bridge disconnected mesh components, leave the mesh and skip an intervening component.','Boundary searches start at the destination and propagate the first corner branch through numerical distance ties. The captured symmetric cases retain the unchanged model epsilon.','The point getter retains one owned spatial index, lazily caches immutable boundary topology, reuses accepted sections and rejects projection misses.'],
  limits=['Boundary routing is bounded to 16384 graph nodes, 1024 components and 4000000 work units; ambiguous or unresolved paths fail before document edits.','The native microscopic degenerate polyline after projection misses is not reproduced.','Cancellation is qualified through the interactive getter; it is not a scripted point argument.','Native point captures use Top. Perspective numeric parity and nonmanifold branches need qualification.','Collinear section vertices may remain; native vertex-count and parameter-clock parity is not claimed.','Graph comparisons treat relative differences within 16 machine epsilons as ties and quantize heap distance to 16 adjacent floating-point values. These sampled native locus comparisons do not certify a universal mesh geodesic or exact shortest path.'],
  sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('logs',type=Path,nargs='+');a=p.parse_args()
 (ROOT/'docs/mesh-polyline-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in a.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
