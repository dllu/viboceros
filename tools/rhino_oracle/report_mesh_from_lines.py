"""Line-network SDK, command and getter qualification with retained token gaps."""
from .qualification_sources import with_shared_sources
import argparse,hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
GROUPS=[('base','MESH_FROM_LINES','',18),('variants','MESH_FROM_LINES_VARIANTS','_variants',16),('controls','MESH_FROM_LINES_CONTROLS','_controls',18),('edges','MESH_FROM_LINES_EDGES','_edges',8)]
SOURCES=[
 'crates/viboceros-geometry/src/mesh/from_lines.rs','crates/viboceros-geometry/src/mesh/from_lines/cycles.rs','crates/viboceros-geometry/src/mesh/from_lines/tests.rs',
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/union_find.rs','crates/viboceros-geometry/src/mesh/patch.rs','crates/viboceros-geometry/src/mesh/patch/constraints.rs','crates/viboceros-geometry/src/mesh/patch/constraints/segments.rs','crates/viboceros-geometry/src/error.rs',
 'crates/viboceros-geometry/src/mesh/normals.rs','crates/viboceros-geometry/src/mesh/radial.rs','crates/viboceros-geometry/src/mesh/projection.rs','crates/viboceros-geometry/src/point_projection.rs','crates/viboceros-geometry/src/point_projection/fit.rs','crates/viboceros-geometry/src/point_projection/fit/normalized.rs','crates/viboceros-geometry/src/frame.rs','crates/viboceros-geometry/src/line.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/mesh_from_lines.rs','crates/viboceros-command/src/object_selection.rs',
 'crates/viboceros-command/tests/mesh_from_lines_native.rs','crates/viboceros-command/tests/support/mesh_from_lines.rs',
 'src/app.rs','src/app/object_selection.rs','src/app/transform_sources.rs','src/app/object_selection/mesh_from_lines.rs','src/app/object_selection/mesh_from_lines/tests.rs',
 'tools/rhino_oracle/mesh_from_lines_probe.py','tools/rhino_oracle/generate_mesh_from_lines.py','tools/rhino_oracle/report_mesh_from_lines.py','tools/rhino_oracle/test_mesh_from_lines.py',
 'tools/rhino_oracle/client.py','tools/rhino_oracle/structural_sdk.py','tools/rhino_oracle/rhino_worker.py',
] + [f'tools/rhino_oracle/{directory}/mesh_from_lines{suffix}.json'for _,_,suffix,_ in GROUPS for directory in ['fixtures','observations']]
SOURCES = with_shared_sources(SOURCES)

def summarize(log):
 groups={};gaps=[]
 for name,prefix,suffix,count in GROUPS:
  q=json.loads((HERE/f'fixtures/mesh_from_lines{suffix}.json').read_text());native=json.loads((HERE/f'observations/mesh_from_lines{suffix}.json').read_text());ids=[o['id']for o in q['operations']]
  if len(ids)!=count or [r['id']for r in native['results']]!=ids:raise ValueError('incomplete native network capture')
  values={r['id']:r['value']for r in native['results']};stages={}
  for stage in ['geometry','command','getter']:
   marker=prefix+'_'+stage.upper();rows=[json.loads(s)for s in re.findall(marker+r' (\{[^\n]+\})',log)]
   if [r['id']for r in rows]!=ids:raise ValueError('incomplete or duplicate '+marker+' replay')
   for row in rows:
    value=values[row['id']];supported='Unknown command:'not in value['history']
    empty=value['sdk']is None if stage=='geometry'else not any(o['role']=='output'for o in value['after_script'])
    if bool(row.get('native_empty'))!=empty:raise ValueError('native empty result hidden')
    error=row.get('maximum_bidirectional_sample_error',0 if empty else math.inf)
    if isinstance(error,bool)or not isinstance(error,(float,int))or not math.isfinite(error)or not 0<=error<1e-6:raise ValueError('invalid line-network comparison')
    if stage=='geometry':
     qualified=error<1e-6
     if not empty:
      sdk=value['sdk']
      if any(type(row[k])is not int or row[k]<=0 for k in ['actual_faces','native_faces','actual_vertices','native_vertices']):raise ValueError('invalid network topology counts')
      if row['native_faces']!=len(sdk['faces'])or row['native_vertices']!=len(sdk['vertices']):raise ValueError('native topology counts changed')
      if row['actual_faces']!=row['native_faces']or row['actual_vertices']!=row['native_vertices']:raise ValueError('line-network topology gap hidden')
    else:
     if row['native_options_supported']is not supported:raise ValueError('rejected native option hidden')
     if row['geometry_qualified']is not True or row['sources_and_attributes_preserved']is not True or row['grouped_history']is not True:raise ValueError('line-network source or history comparison omitted')
     qualified=error<1e-6 and supported
    if row['qualified']is not qualified:raise ValueError('network qualification gap hidden')
   stages[stage]=dict(recipes=len(rows),qualified_recipes=sum(r['qualified']for r in rows),unqualified_recipes=[r['id']for r in rows if not r['qualified']],cases=rows)
  for id_,value in values.items():
   if 'Unknown command:'in value['history']:gaps.append(id_)
  groups[name]=dict(native_engine_version=native['engine_version'],**stages)
 return dict(model_epsilon=1e-6,full_native_parity=False,performance_qualified=False,recipes=sum(g[3]for g in GROUPS),native_inline_option_gaps=gaps,
  maximum_sample_difference=max(r.get('maximum_bidirectional_sample_error',0)for g in groups.values()for stage in ['geometry','command','getter']for r in g[stage]['cases']),groups=groups,
  guarantees=[
   'All 60 fixed SDK inputs match bidirectional mesh samples and vertex/face counts at the unchanged 1e-6 gate.',
   'Command and staged getter replays match effective accepted settings, output geometry, attributes, source snapshots, selection, deletion and grouped Undo/Redo. Rejected native inline spellings remain unqualified token rows.',
   'Endpoint clustering uses the supplied SDK tolerance or the captured command document tolerance. Interior line crossings are not split. Open chains and dangling branches do not form mesh faces.',
   'Triangles and independent longer chordless graph cycles recover three-dimensional regions; larger polygons reuse constrained patch triangulation. Command meshes unweld at the captured 60-degree threshold.',
   'Construction is capped at 100000 line records and 4000000 work units. Dense graphs fail before admission; 5000 disconnected triangular regions exercise linear output charging.',
   'Preferences belong to the command registry outside model Undo. Staged cancellation preserves Redo, and changed source/history state rejects stale construction.',
  ],limits=[
   'Sampling and vertex/face counts do not certify universal face order, orientation, or unique topology for ambiguous networks. A recovered cycle set need not be a closed shell.',
   'The captured native inline CurrentLayer, Input, source-name and numeric output-layer spellings are rejected. Accepted Current-layer settings are replayed, and unsupported token rows remain explicit. Local layer-ID targeting is an application extension.',
   'The initial near_endpoint/far_endpoint and gap_* setup moved coincident endpoints together. Those immutable closed-shape captures are retained; actual_gap_* and actual_sdk_* controls independently move only one endpoint.',
   'Two controls deliberately supply different SDK and document tolerances. Their SDK and command results are compared against their corresponding inputs.',
   'General nonmanifold and nonorientable networks, nonplanar quad tessellation, projection-ambiguous large polygons, input-order effects beyond the captures, and native comparative speed require further qualification.',
  ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
 (ROOT/'docs/mesh-from-lines-qualification.json').write_text(json.dumps(summarize('\n'.join(p.read_text()for p in args.logs)),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
