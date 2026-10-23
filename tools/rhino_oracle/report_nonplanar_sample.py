"""Qualify acknowledged native face-threshold getters and preserve failed batches."""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from .nonplanar_sample_cases import CASES,request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE,ROOT,error
FAMILIES={'nonplanar_sample_shaded':('shaded',2),'nonplanar_sample_core':('core',14),'nonplanar_sample_rules':('rules',5),'nonplanar_sample_external':('external',1),'nonplanar_sample_raw':('raw',1),'nonplanar_sample_neighbors':('neighbors',2)}
SOURCES=with_shared_sources([
    'Cargo.lock','src/app.rs','src/app/component_selection.rs','src/app/prompt_cancel.rs',
    'src/app/object_selection/mesh_conversion.rs','src/app/object_selection/mesh_conversion/face_sample.rs','src/app/object_selection/mesh_conversion/face_sample_tests.rs',
    'src/viewport.rs','src/viewport/component_picking.rs','src/viewport/picking.rs','src/viewport/polygon_components/tests.rs',
    'crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/triangulate_nonplanar_quads.rs','crates/viboceros-command/tests/support/nonplanar_sample.rs',
    'crates/viboceros-command/tests/support/crease.rs','crates/viboceros-command/tests/support/mesh_command.rs','crates/viboceros-command/tests/support/mesh_conversion.rs',
    'crates/viboceros-command/tests/support/subd.rs','crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-geometry/src/lib.rs','crates/viboceros-geometry/src/error.rs','crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/quad_planarity.rs',
    'crates/viboceros-opennurbs/src/mesh.rs','crates/viboceros-opennurbs/native/mesh_modeling.cpp','crates/viboceros-opennurbs/native/mesh_modeling.h','crates/viboceros-opennurbs/native/mesh_codec.h',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h','third_party/opennurbs/opennurbs_point.h',
]+['tools/rhino_oracle/'+p for p in [
    'nonplanar_sample_cases.py','nonplanar_sample_probe.py','generate_nonplanar_sample.py','report_nonplanar_sample.py','test_nonplanar_sample.py',
    'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py','client.py','structural_sdk.py','rhino_worker.py',
    'fixtures/nonplanar_sample_discovery.json','observations/nonplanar_sample_discovery.json',
    'fixtures/nonplanar_sample_main.json','observations/nonplanar_sample_main_timeout.json','observations/nonplanar_sample_main_timeout.log',
    'fixtures/nonplanar_sample_main_v2.json','observations/nonplanar_sample_main_interrupted.json','observations/nonplanar_sample_main_interrupted.log','observations/nonplanar_sample_main_interrupted_progress.log',
    'fixtures/nonplanar_sample_quad_ngon.json','observations/nonplanar_sample_quad_ngon_timeout.json','observations/nonplanar_sample_quad_ngon_timeout.log',
]])


def options(history,before):
    result=dict(before);found=False
    for mode,threshold,number,method in re.findall(r'Change option or press Enter to accept \( SelectBy=(Distance|Angle)\s+(Distance|Angle)=([\d.eE+-]+)\s+SplitMethod=(\w+)',history):
        if mode!=threshold:raise ValueError('native sampling criterion differs')
        if not found and (mode!=before['select_by']or float(number)!=before[threshold.lower()]or method!=before['split_method']):raise ValueError('native sampling preferences changed')
        found=True;result.update(select_by=mode,split_method=method);result[threshold.lower()]=float(number)
    if not found:raise ValueError('missing native threshold prompt')
    return result


def native():
    result={};versions=set()
    for family,(flag,count)in FAMILIES.items():
        data=json.loads((HERE/'observations'/(family+'.json')).read_text());versions.add(data['engine_version'])
        query=request(**{flag:True})
        if json.loads((HERE/'fixtures'/(family+'.json')).read_text())!=query or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in query['operations']]:raise ValueError('missing or changed native face inputs')
        before=dict(select_by='Distance',distance=0.,angle=0.,split_method='ShortestDiagonal')
        for row in data['results']:
            v=row.get('value')
            if v is None or v['input']!=CASES[row['id']]:raise ValueError('native face recipe changed')
            events=[e for e in v['events']if e['name']=='TriangulateNonPlanarQuads']
            accepted=not v['input'].get('sample_cancel',False)
            if len(events)!=1 or v['success']is not accepted or (events[0]['result']=='Success')is not accepted:raise ValueError('native sampling admission differs')
            actions=v['input'].get('actions',[]);clicks=[i for i,a in enumerate(actions)if isinstance(a,dict)]
            traces=v['input_trace']
            if actions:
                if len(traces)!=1 or traces[0]['errors']or traces[0]['completed']!=len(actions)or traces[0]['expected']!=len(actions):raise ValueError('incomplete native face input')
                trace=traces[0]
                if [c['action']for c in trace.get('clicks',[])]!=clicks:raise ValueError('missing native face click')
                for c in trace.get('clicks',[]):
                    i=c['action'];ray=c['ray']
                    if c['motion_acknowledged']is not True or c['click_acknowledged']is not True or c['aim']!=actions[i]['click']or not trace['prompts'][i].startswith('Select mesh face'):
                        raise ValueError('unacknowledged native face pick')
                    if len(ray)!=2 or any(len(p)!=3 or any(type(x)not in(int,float)or not math.isfinite(x)for x in p)for p in ray)or ray[0]==ray[1]:raise ValueError('invalid independent native click ray')
                    source=v['input']['sources'][v['input'].get('sample_source',0)];face=source['faces'][v['input']['sample_face']]
                    points=[source['vertices'][j]for j in face];point=ray[0]
                    def cross(a,b,p):return (b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0])
                    def inside(a,b,c):
                        area=cross(a,b,c)
                        return area!=0. and all(cross(x,y,point)*area>=-1e-12*area*area for x,y in [(a,b),(b,c),(c,a)])
                    if len(face)!=4 or abs(ray[0][0]-ray[1][0])>1e-9 or abs(ray[0][1]-ray[1][1])>1e-9 or not(inside(*points[:3])or inside(points[0],points[2],points[3])):raise ValueError('native click does not witness the input quad')
            elif traces:raise ValueError('unexpected native face trace')
            staged=options(v['history'],before)
            if accepted:before=staged
            result[row['id']]=(v,dict(before),len(clicks))
    if len(versions)!=1:raise ValueError('mixed face sampler engines')
    return versions.pop(),result


def summarize(text):
    version,expected=native();rows=[json.loads(s)for s in re.findall(r'^FACE_SAMPLER (.+)$',text,re.M)]
    if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate face replay')
    maximum=distance=angle=0.
    for r in rows:
        v,pref,picks=expected[r['id']]
        for flag in ['qualified','geometry','channels','identity','metadata','selection','object_order','history','staged']:
            if r.get(flag)is not True:raise ValueError('unqualified face sampler '+flag)
        if type(r.get('face_picks'))is not int or r['face_picks']!=picks or r.get('raw_face_picker')is not bool(picks)or r.get('accepted')is not v['success']:raise ValueError('native face input relabelled')
        actual=r.get('preferences')
        if not isinstance(actual,dict)or set(actual)!=set(pref)or actual['select_by']!=pref['select_by']or actual['split_method']!=pref['split_method']:raise ValueError('native sampling options differ')
        for key,field in [('distance','maximum_distance_error'),('angle','maximum_angle_error_degrees')]:
            value=actual[key];delta=r.get(field)
            if type(value)not in(int,float)or not math.isfinite(value)or type(delta)not in(int,float)or not math.isfinite(delta)or not 0.<=delta<=1e-10 or delta!=abs(value-pref[key]):raise ValueError('incorrect threshold precision evidence')
        maximum=max(maximum,error(r.get('maximum_error')));distance=max(distance,r['maximum_distance_error']);angle=max(angle,r['maximum_angle_error_degrees'])
    rows.sort(key=lambda r:list(expected).index(r['id']))
    paths=list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        paths.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_version=version,model_epsilon=1e-6,threshold_epsilon=1e-10,native_workflows=len(rows),face_samples=sum(r['face_picks']for r in rows),getter_cases=len(rows),
        accepted_commands=sum(r['accepted']for r in rows),maximum_error=maximum,maximum_distance_error=distance,maximum_angle_error_degrees=angle,
        full_native_parity=False,performance_qualified=False,discovery_qualified_workflows=0,discovery_incomplete_workflows=4,
        failed_batch_requested_workflows=33,failed_batch_qualified_workflows=0,cases=rows,
        limits=[
            'Independent public command recipes use owned shaded Top viewports and public mouse-move/down acknowledgements. Exact saved source archives and captured world rays drive local viewport mouse events, stored face picks, typed command admission, geometry/vertex channels, attributes, IDs, groups, selection, object order and Undo/Redo.',
            'OpenNURBS measures the diagonal-normal face-plane distance range and largest opposite-corner angle using only four double-precision points. Sampling updates the active threshold, preserving the other value. Native subtraction steps follow decimal magnitude; exact powers, scaled geometry, reversed/cyclic faces and small angles are independent holdouts.',
            'Flat sampled values persist as -0.01 and disable the active test, leaving geometry unchanged. Explicit numeric negatives clamp to zero. Typed acceptance preserves sampled values without reparsing numeric entry. Nested Escape/Cancel returns to options; another cancels. Enter keeps a current threshold. Cancel, stale inputs and invalid picks create no geometry edit.',
            'Absolute tolerances from 1e-6 to 0.1 and angular tolerances from 1e-6 to 0.5 radians do not change sampled values. An unselected external mesh supplies a threshold without becoming a conversion target. A separate two-quad case verifies stored face one rather than the first face.',
            'Four initial wireframe probes have timeout/incomplete input traces and qualify no workflows. Two multi-case attempts and an isolated quad n-gon attempt stop at an n-gon pick; none retains a batch response, so no geometry from those thirty-three requested workflows is qualified. Independent recaptures qualify only their own completed records. Logs, requests and frozen producers survive.',
            'Sampling on n-gon meshes, native wireframe/ghosted input, dialog previews, spinner controls, other views/units, degenerate source repair, restricted objects, multi-target ordering, numeric extremes and comparative speed remain unqualified. Local raw face selection inside n-gons is covered independently, without claiming native command parity.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('log',type=Path);args=parser.parse_args()
    data=summarize(args.log.read_text());(ROOT/'docs/nonplanar-sample-qualification.json').write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
