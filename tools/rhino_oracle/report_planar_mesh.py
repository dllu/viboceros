"""PlanarMesh surface and topology qualification across both geometry adapters."""
from .qualification_sources import with_application_sources
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
SOURCES=[
 'crates/viboceros-command/src/planar_mesh.rs','crates/viboceros-command/src/planar_surface.rs',
 'crates/viboceros-command/src/planar_surface/native.rs','crates/viboceros-command/src/planar_surface/portable.rs',
 'crates/viboceros-command/src/lib.rs','crates/viboceros-command/tests/planar_mesh_native.rs',
 'crates/viboceros-command/tests/support/planar_mesh.rs','src/app/object_selection.rs',
 'src/app/object_selection/planar_mesh_tests.rs',
 'crates/viboceros-geometry/src/brep.rs','crates/viboceros-geometry/src/brep/tessellation.rs',
 'crates/viboceros-geometry/src/brep/tessellation/planar_mesh.rs',
 'crates/viboceros-geometry/src/mesh/patch.rs','crates/viboceros-geometry/src/mesh/patch/constraints.rs',
 'crates/viboceros-geometry/src/mesh/patch/constraints/segments.rs',
 'crates/viboceros-geometry/src/curve_join.rs','crates/viboceros-geometry/src/curve_join/assembly.rs',
 'crates/viboceros-geometry/src/curve_join/search.rs','crates/viboceros-geometry/src/curve_join/seeded.rs',
 'crates/viboceros-geometry/src/polyline/reduction.rs','crates/viboceros-geometry/src/frame.rs',
 'crates/viboceros-geometry/src/mesh/projection.rs','crates/viboceros-smlib/src/native/construction.rs',
 'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/append.rs',
 'crates/viboceros-geometry/src/point_projection.rs','crates/viboceros-geometry/src/point_projection/fit.rs',
 'crates/viboceros-geometry/src/point_projection/fit/normalized.rs',
 'crates/viboceros-geometry/src/nurbs.rs','crates/viboceros-geometry/src/nurbs/intersection_backend.rs',
 'crates/viboceros-smlib/native/bridge.cpp',
 'tools/rhino_oracle/planar_mesh_probe.py','tools/rhino_oracle/generate_planar_mesh.py',
 'tools/rhino_oracle/report_planar_mesh.py','tools/rhino_oracle/test_planar_mesh.py',
 'tools/rhino_oracle/client.py','tools/rhino_oracle/structural_sdk.py','tools/rhino_oracle/rhino_worker.py',
 'tools/rhino_oracle/fixtures/planar_mesh.json','tools/rhino_oracle/observations/planar_mesh.json',
 'tools/rhino_oracle/fixtures/planar_mesh_postselect.json','tools/rhino_oracle/observations/planar_mesh_postselect.json',
]
SOURCES = with_application_sources(SOURCES)


def native_rows(suffix='',count=21):
    request=json.loads((HERE/f'fixtures/planar_mesh{suffix}.json').read_text())
    response=json.loads((HERE/f'observations/planar_mesh{suffix}.json').read_text())
    ids=[op['id']for op in request['operations']]
    if len(ids)!=count or len(set(ids))!=count or [row['id']for row in response['results']]!=ids:
        raise ValueError('incomplete native PlanarMesh capture')
    return response['results']


def check(row,value):
    c=row['check'];targets=[o['mesh']for o in value['after_script']if o['role']=='output']
    if any(type(c.get(key))is not int or c[key]<0 for key in ['actual_meshes','native_meshes']):
        raise ValueError('invalid output mesh counts')
    if c['native_meshes']!=len(targets):raise ValueError('native output count changed')
    indices=[];errors=[];counts=[]
    if len(c['meshes'])!=min(c['actual_meshes'],len(targets)):raise ValueError('missing mesh comparison')
    for mesh in c['meshes']:
        index=mesh['native_index']
        if type(index)is not int or not 0<=index<len(targets)or index in indices:
            raise ValueError('missing or reused native mesh')
        indices.append(index);target=targets[index]
        for key in ['actual_vertices','actual_faces','actual_triangles','native_vertices','native_faces','native_triangles']:
            if type(mesh.get(key))is not int or mesh[key]<0:raise ValueError('invalid mesh topology counts')
        if (mesh['native_vertices'],mesh['native_faces'],mesh['native_triangles'])!=(len(target['vertices']),len(target['faces']),sum(len(f)==3 for f in target['faces'])):
            raise ValueError('native mesh topology changed')
        same=all(mesh['actual_'+key]==mesh['native_'+key]for key in ['vertices','faces','triangles'])
        if mesh['counts_match']is not same:raise ValueError('hidden topology gap')
        counts.append(same);error=mesh.get('maximum_bidirectional_sample_error')
        if type(error)not in (float,int)or not math.isfinite(error)or error<0:raise ValueError('invalid surface distance')
        errors.append(error)
    complete=c['actual_meshes']==len(targets)
    maximum=max(errors,default=0)if complete else None
    reported=c.get('maximum_bidirectional_sample_error')
    if maximum is None:
        if reported is not None:raise ValueError('unmatched surfaces have no comparable distance')
    elif type(reported)not in (float,int)or not math.isfinite(reported)or maximum!=reported:
        raise ValueError('invalid aggregate distance')
    geometry=complete and all(e<=1e-6 for e in errors)
    topology=complete and all(counts)
    if c['geometry_qualified']is not geometry or c['counts_match']is not topology or c['qualified']is not (geometry and topology):
        raise ValueError('hidden surface or topology gap')
    if c['sources_and_attributes_preserved']is not True or row['grouped_history']is not True:
        raise ValueError('source or history replay missing')
    return geometry,topology


def summarize(native_log,portable_log,getter_log):
    native=native_rows();ids=[row['id']for row in native];values={r['id']:r['value']for r in native}
    controls=native_rows('_postselect',3);control_values={r['id']:r['value']for r in controls}
    stages={}
    for stage,prefix,log in [('native','COMMAND',native_log),('portable','COMMAND',portable_log),
                             ('preselected','PRESELECTED',getter_log),('postselected','POSTSELECTED',getter_log)]:
        rows=[json.loads(s)for s in re.findall(r'PLANAR_MESH_'+prefix+r' (\{[^\n]+\})',log)]
        if [r['id']for r in rows]!=ids:raise ValueError('incomplete or duplicate '+stage+' replay')
        for row in rows:
            target_id=('post_'+row['id']if 'post_'+row['id']in control_values else row['id'])if stage=='postselected'else row['id']
            if stage in ('preselected','postselected')and row.get('target_id')!=target_id:raise ValueError('selection target changed')
            value=control_values[target_id]if target_id in control_values else values[target_id]
            geometry,topology=check(row,value)
            failed=any(e['name']=='PlanarMesh'and e['result']=='Failure'for e in value['events'])
            if stage in ('native','portable'):
                status=(row.get('local_error')is not None)==failed
            else:
                if row.get('native_failed')is not failed or type(row.get('local_failed'))is not bool:
                    raise ValueError('native command failure changed')
                status=row['local_failed']==failed
            if row['native_status_match']is not status or row['geometry_qualified']is not (geometry and status)or row['qualified']is not (geometry and topology and status):
                raise ValueError('hidden command outcome gap')
        stages[stage]=dict(recipes=len(rows),geometry_qualified_recipes=sum(r['geometry_qualified']for r in rows),
            qualified_recipes=sum(r['qualified']for r in rows),
            topology_gaps=[r['id']for r in rows if not r['check']['counts_match']],
            outcome_gaps=[r['id']for r in rows if not r['geometry_qualified']],cases=rows)
    control_rows=[json.loads(s)for s in re.findall(r'PLANAR_MESH_POSTCONTROL (\{[^\n]+\})',getter_log)]
    if [r['id']for r in control_rows]!=[r['id']for r in controls]:raise ValueError('incomplete postselection control replay')
    for row in control_rows:
        value=control_values[row['id']];geometry,topology=check(row,value)
        failed=any(e['name']=='PlanarMesh'and e['result']=='Failure'for e in value['events'])
        if row['local_failed']is not failed or row['native_failed']is not failed or row['native_status_match']is not True:
            raise ValueError('postselection failure state changed')
        if row['qualified']is not (geometry and topology)or row['geometry_qualified']is not geometry:raise ValueError('hidden postselection control gap')
    return dict(model_epsilon=1e-6,recipes=21,full_native_parity=False,performance_qualified=False,
        postselection_controls=dict(recipes=3,qualified_recipes=sum(r['qualified']for r in control_rows),cases=control_rows),
        native_engine_version=json.loads((HERE/'observations/planar_mesh.json').read_text())['engine_version'],stages=stages,
        guarantees=[
            'All 21 captured command outcomes match surface samples and source/history checks in native and portable builds, with preselected and postselected UI replays.',
            'Holes, nested islands, disconnected and overlapping regions, different planes, closed line chains, and retained source attributes have native observations.',
            'usd-brep admits native curves and builds noncontacting planar regions. Existing portable construction handles boundary contacts and upstream planar-face rejection after native admission.',
            'Rectangular regions use the shared four-by-four quad grid. Other regions reuse constrained patch triangulation with sixteen samples per trim span.',
            'The command bounds input to 128 profiles and 100000 joined control points. Each planar B-rep samples at most 100000 boundary stations before allocation; constrained recovery retains shared limits.',
        ],limits=[
            'Eleven captured cases differ in raw vertex/face/triangle counts. Surface sample matches do not imply native interior tessellation, topology, face ordering, or universal shape parity.',
            'The two rejected nonplanar/open cases compare native failure, no output and unchanged history. Three dedicated native postselection controls retain selection release on failure; preselected failure retains selection.',
            'Captured single-boundary SDK meshes are preserved as references. They use different settings from the command and are not substituted for multi-boundary command targets.',
            'Sixteen samples per trim span match the captured circles and ellipse within the model gate; general adaptive meshing and native comparative speed need further qualification.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',type=Path,required=True);parser.add_argument('--portable',type=Path,required=True);parser.add_argument('--getters',type=Path,required=True)
    args=parser.parse_args()
    (ROOT/'docs/planar-mesh-qualification.json').write_text(json.dumps(summarize(args.native.read_text(),args.portable.read_text(),args.getters.read_text()),indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
