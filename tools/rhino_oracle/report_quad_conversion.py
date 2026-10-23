"""Qualify public SDK geometry conversion and the local command lifecycle."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .report_subd_box import HERE, ROOT, error
from .qualification_sources import with_shared_sources
from .quad_conversion_cases import CASES, request
FAMILY='mesh_quad_conversion_v3'
SOURCES=with_shared_sources([
    'Cargo.lock','crates/viboceros-command/src/lib.rs','crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/src/mesh/native.rs',
    'crates/viboceros-opennurbs/src/mesh.rs','crates/viboceros-opennurbs/native/mesh_modeling.cpp',
    'crates/viboceros-opennurbs/native/mesh_modeling.h','crates/viboceros-opennurbs/native/mesh_codec.h',
    'crates/viboceros-command/src/quadrangulate_mesh.rs','crates/viboceros-command/tests/quad_conversion_native.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp',
]+['tools/rhino_oracle/'+p for p in [
    'quad_conversion_cases.py','quad_conversion_probe.py','generate_quad_conversion.py','mesh_channel_capture.py',
    'report_quad_conversion.py','test_quad_conversion.py','rhino_worker.py','structural_sdk.py','client.py',
    'fixtures/mesh_quad_conversion.json','observations/mesh_quad_conversion_dispatch_failure.json',
    'observations/mesh_quad_conversion_dispatch_failure.log',
    'fixtures/mesh_quad_conversion_v2.json','observations/mesh_quad_conversion_v2.json',
    'fixtures/mesh_quad_conversion_v3.json','observations/mesh_quad_conversion_v3.json',
]])


def native():
    query=json.loads((HERE/'fixtures'/(FAMILY+'.json')).read_text())
    data=json.loads((HERE/'observations'/(FAMILY+'.json')).read_text())
    if query!=request()or [r['id']for r in data['results']]!=list(CASES):
        raise ValueError('missing or reordered quad conversion references')
    for r in data['results']:
        v=r.get('value')
        if v is None or v['input']!=CASES[r['id']]or v['valid_before']is not True or v['valid_after']is not True:
            raise ValueError('invalid quad conversion reference')
    return data


def summarize(text):
    data=native();expected={r['id']:r['value']for r in data['results']}
    rows=[json.loads(s)for s in re.findall(r'^QUAD_CONVERSION (.+)$',text,re.M)]
    if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):
        raise ValueError('missing or duplicate quad conversion replay')
    maximum=0.
    for r in rows:
        v=expected[r['id']];pairs=len(v['before']['faces'])-len(v['after']['faces'])
        for flag in ['qualified','history','metadata','selection','channels','raw_faces']:
            if r.get(flag)is not True:raise ValueError('unqualified conversion '+flag)
        if r.get('changed')is not v['changed']:raise ValueError('incorrect conversion history')
        invalid=[0]if r['id']=='quad_convert_adjacent_ngons'else[]
        if r.get('native_invalid_ngons')!=invalid or r.get('native_ngons_correspond')is not (not invalid):
            raise ValueError('invalid native n-gon difference relabelled')
        for key,wanted in [('command_syntaxes',2),('pairs',pairs),('faces',len(v['after']['faces'])),('vertices',len(v['after']['vertices'])),('ngons',len(v['after']['ngons'])-len(invalid))]:
            if type(r.get(key))is not int or r[key]!=wanted:raise ValueError('conversion count differs')
        maximum=max(maximum,error(r.get('maximum_error')))
    rows.sort(key=lambda r:list(expected).index(r['id']))
    return dict(native_engine_version=data['engine_version'],model_epsilon=1e-6,
        sdk_geometry_cases=len(rows),local_command_cases=len(rows),maximum_error=maximum,
        native_invalid_overlay_cases=['quad_convert_adjacent_ngons'],native_command_prompt_qualified=False,
        performance_qualified=False,full_native_parity=False,earlier_geometry_only_references=14,
        failed_dispatch_qualified_cases=0,cases=rows,
        limits=[
            'Fourteen independently constructed native SDK inputs match raw vertices, face records and channels exactly. The registered command replays both equals and separate-token explicit options and verifies identity, attributes, groups, selection, Undo/Redo; accepted no-ops create Undo and clear redo.',
            'OpenNURBS performs conversion directly, including strict mutual-longest-edge selection on meshes with more than two faces, angle normalization, raw seam preservation and native face ordering. Source vertex and channel storage remain intact.',
            'Native conversion may invalidate an n-gon while Mesh.IsValid remains true. The adjacent-ngon reference exposes a three-vertex boundary attached to a quad; Viboceros discards this invalid overlay. That metadata difference remains explicit.',
            'These SDK inputs do not qualify command prompts/defaults; a separate native command record covers those workflows. Inputs use valid stored geometry and ordinary representable coordinates; extreme float32 selection behavior and comparative performance remain unqualified. Native packing is bounded to four million vertices and one million faces.',
            'An earlier capture has geometry-only snapshots. The initial worker dispatch failure retains its request, log and frozen producers; no native geometry from that failed attempt is qualified.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('log',type=Path);args=parser.parse_args()
    (ROOT/'docs/quad-conversion-qualification.json').write_text(json.dumps(summarize(args.log.read_text()),indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
