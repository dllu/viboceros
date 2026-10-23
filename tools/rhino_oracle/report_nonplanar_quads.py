"""Qualify selective quad triangulation against independent native SDK inputs."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .nonplanar_quad_cases import request, CASES
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error
FAMILIES={'nonplanar_quad_conversion_v2':32,'nonplanar_quad_ngons':4,'nonplanar_quad_angles':6}
SOURCES=with_shared_sources([
    'Cargo.lock','crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/triangulate_nonplanar_quads.rs',
    'crates/viboceros-command/tests/nonplanar_quad_native.rs','crates/viboceros-command/tests/support/mesh_conversion.rs',
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/nonplanar_quads.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs',
    'crates/viboceros-opennurbs/src/mesh.rs','crates/viboceros-opennurbs/native/mesh_modeling.cpp',
    'crates/viboceros-opennurbs/native/mesh_modeling.h','crates/viboceros-opennurbs/native/mesh_codec.h',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h',
]+['tools/rhino_oracle/'+p for p in [
    'nonplanar_quad_cases.py','nonplanar_quad_probe.py','generate_nonplanar_quads.py','mesh_channel_capture.py',
    'report_nonplanar_quads.py','test_nonplanar_quads.py','qualification_sources.py','client.py','rhino_worker.py','structural_sdk.py',
    'fixtures/nonplanar_quad_conversion.json','observations/nonplanar_quad_import_failure.json','observations/nonplanar_quad_import_failure.log',
]])


def native():
    result={};versions=set()
    for family,count in FAMILIES.items():
        query=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        data=json.loads((HERE/'observations'/(family+'.json')).read_text());versions.add(data['engine_version'])
        if query!=request(ngons=family.endswith('ngons'),angles=family.endswith('angles'))or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in query['operations']]:
            raise ValueError('missing or changed nonplanar quad references')
        for row in data['results']:
            v=row.get('value')
            if v is None or v['input']!=CASES[row['id']]or v['before']['valid']is not True or v['after']['valid']is not True:
                raise ValueError('invalid nonplanar quad reference')
            result[row['id']]=v
    if len(versions)!=1:raise ValueError('mixed native mesh versions')
    return versions.pop(),result


def summarize(text):
    version,expected=native()
    rows=[json.loads(s)for s in re.findall(r'^NONPLANAR_QUAD (.+)$',text,re.M)]
    if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate quad replay')
    maximum=0.
    for r in rows:
        v=expected[r['id']];target=v['after']
        for flag in ['qualified','raw_faces','channels','metadata','selection','history']:
            if r.get(flag)is not True:raise ValueError('unqualified nonplanar quad '+flag)
        empty=[i for i,g in enumerate(target['ngons'])if not g['faces']and not g['vertices']]
        if r.get('native_invalid_ngons')!=[]or r.get('native_empty_ngon_slots')!=empty:raise ValueError('native n-gon slots relabelled')
        for key,wanted in [('converted',v['converted']),('vertices',len(target['vertices'])),('faces',len(target['faces'])),('ngons',len(target['ngons'])-len(empty))]:
            if type(r.get(key))is not int or r[key]!=wanted:raise ValueError('incorrect nonplanar quad counts')
        maximum=max(maximum,error(r.get('maximum_error')))
    rows.sort(key=lambda r:list(expected).index(r['id']))
    paths=list(SOURCES)
    for family in FAMILIES:paths.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
    return dict(native_engine_version=version,model_epsilon=1e-6,sdk_geometry_cases=len(rows),local_command_cases=len(rows),
        maximum_error=maximum,full_native_parity=False,native_command_workflow_qualified=False,performance_qualified=False,
        failed_import_requested_workflows=32,failed_import_qualified_workflows=0,cases=rows,
        limits=[
            'Forty-two independent public SDK inputs match raw vertices, face records, stored colors/UV/normals and logical n-gon overlays exactly. Local registered commands verify identity, attributes, groups, selection and Undo/Redo, including unchanged-input redo preservation. These command history invariants are not a native UI comparison.',
            'OpenNURBS selects nonplanar quads using its face-plane distance range and both opposite corner-normal pairs, independently of the split method. Exceeding either enabled threshold splits a quad. Strict distance equality and adjacent representable limits, zero thresholds, all six split methods, angular holdouts between the two diagonal-normal angles, concave inputs, translated/tiny geometry and unused vertices are covered.',
            'RhinoCommon removes n-gons containing a split quad; the binding requests that policy after initializing its face-to-ngon map. Four extra scenes vary n-gon ownership order and preserve unaffected overlays. Native empty removed slots are recorded explicitly and omitted from the model overlay vector.',
            'The geometry operation uses mesh validation tolerance independently of document tolerance. The first native attempt failed during import because IronPython lacks math.nextafter. Its request, host log and frozen producers survive; no response or geometry from that attempt is qualified.',
            'Native command prompts, defaults, option memory, selection/history policy, restricted objects, grips, degenerate-input repair, split method zero/default, coordinates near numeric limits and comparative performance remain unqualified. The binding limits source input to four million vertices and one million faces.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('log',type=Path);args=parser.parse_args()
    report=summarize(args.log.read_text())
    (ROOT/'docs/nonplanar-quad-qualification.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
