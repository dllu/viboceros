"""Qualify native selective quad commands, retaining rejected discovery syntax."""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from .nonplanar_command_cases import CASES,request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE,ROOT,error
FAMILIES={'nonplanar_command_discovery':3,'nonplanar_command_options':4,'nonplanar_command_syntax_v2':6,'nonplanar_command_main':20,'nonplanar_command_selection':2}
EXCLUDED={
    'nonplanar_command_both_default':'Native scripted SelectBy rejects Both; it leaves Angle active. This is not evidence for the local combined criterion.',
    'nonplanar_command_explicit_distance':'Inline Distance=value enters a face getter without admitting that numeric value; a later split option is rejected and the command cancels.',
    'nonplanar_command_distance_separate':'Distance was entered while Angle was active. It invokes the separate Distance measurement command, not a triangulation option.',
    'nonplanar_command_combined_name':'Native scripted SelectBy rejects DistanceAndAngle; it leaves Angle active.',
    'nonplanar_command_first_selection':'All already advances the native single-mesh picker. The extra Enter accepted the command before the intended options, which were rejected after it ended.',
}
SOURCES=with_shared_sources([
    'Cargo.lock','src/app.rs','crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/triangulate_nonplanar_quads.rs','crates/viboceros-command/src/remembered.rs',
    'crates/viboceros-command/tests/nonplanar_command_native.rs','crates/viboceros-command/tests/support/nonplanar_command.rs',
    'crates/viboceros-command/tests/support/crease.rs','crates/viboceros-command/tests/support/mesh_command.rs',
    'crates/viboceros-command/tests/support/mesh_conversion.rs','crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs','crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-opennurbs/src/mesh.rs','crates/viboceros-opennurbs/native/mesh_modeling.cpp',
    'crates/viboceros-opennurbs/native/mesh_modeling.h','crates/viboceros-opennurbs/native/mesh_codec.h',
    'third_party/opennurbs/opennurbs_mesh.cpp','third_party/opennurbs/opennurbs_mesh.h',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
]+['tools/rhino_oracle/'+p for p in [
    'nonplanar_command_cases.py','nonplanar_command_probe.py','generate_nonplanar_command.py',
    'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
    'client.py','rhino_worker.py','structural_sdk.py','report_nonplanar_command.py','test_nonplanar_command.py',
    'fixtures/nonplanar_command_syntax.json','observations/nonplanar_command_import_failure.json','observations/nonplanar_command_import_failure.log',
]])


def native():
    result={};versions=set()
    for family,count in FAMILIES.items():
        data=json.loads((HERE/'observations'/(family+'.json')).read_text());versions.add(data['engine_version'])
        wanted=request(options=family.endswith('options'),syntax=family.endswith('syntax_v2'),main=family.endswith('main'),selection=family.endswith('selection'))
        if json.loads((HERE/'fixtures'/(family+'.json')).read_text())!=wanted or len(data['results'])!=count or [r['id']for r in data['results']]!=[r['id']for r in wanted['operations']]:
            raise ValueError('missing or changed native nonplanar workflows')
        preferences=dict(select_by='Distance',distance=0.,angle=0.,split_method='ShortestDiagonal')
        for row in data['results']:
            value=row.get('value')
            if value is None or value['input']!=CASES[row['id']]:raise ValueError('changed nonplanar command input')
            events=[e for e in value['events']if e['name']=='TriangulateNonPlanarQuads']
            if len(events)!=1 or type(value['success'])is not bool or (events[0]['result']=='Success')is not value['success']:
                raise ValueError('unexpected native command outcome')
            prompts=re.findall(r'Change option or press Enter to accept \( SelectBy=(Distance|Angle)\s+(Distance|Angle)=([\d.eE+-]+)\s+SplitMethod=(\w+)',value['history'])
            if prompts:
                mode,threshold,number,method=prompts[0]
                if mode!=threshold or mode!=preferences['select_by'] or float(number)!=preferences[threshold.lower()] or method!=preferences['split_method']:
                    raise ValueError('native nonplanar preferences changed')
            if value['success']:
                if not prompts:raise ValueError('missing native conversion prompt')
                for mode,threshold,number,method in prompts:
                    if mode!=threshold:raise ValueError('inconsistent native criterion')
                    preferences.update(select_by=mode,split_method=method)
                    preferences[threshold.lower()]=float(number)
            result[row['id']]=(value,dict(preferences))
    if len(versions)!=1:raise ValueError('mixed native versions')
    if not set(EXCLUDED)<=set(result):raise ValueError('missing discovery exclusions')
    return versions.pop(),result


def summarize(text):
    version,reference=native();qualified={k:v for k,v in reference.items()if k not in EXCLUDED};stages={};maximum=0.
    for stage in ['COMMAND','GETTER']:
        expected={k:v for k,v in qualified.items()if stage=='GETTER'or v[0]['success']}
        rows=[json.loads(s)for s in re.findall(r'^NONPLANAR_'+stage+r' (.+)$',text,re.M)]
        if len(rows)!=len(expected)or {r['id']for r in rows}!=set(expected):raise ValueError('missing or duplicate nonplanar command replay')
        for row in rows:
            value,preferences=expected[row['id']]
            for flag in ['qualified','geometry','channels','identity','metadata','object_order','selection','history']:
                if row.get(flag)is not True:raise ValueError('unqualified nonplanar workflow '+flag)
            actual=row.get('preferences')
            if not isinstance(actual,dict)or set(actual)!=set(preferences)or any(type(actual[k])not in(int,float)or not math.isfinite(actual[k])for k in ['distance','angle']):
                raise ValueError('invalid nonplanar preferences')
            if row.get('accepted')is not value['success']or actual!=preferences:raise ValueError('native admission or option memory differs')
            maximum=max(maximum,error(row.get('maximum_error')))
        stages[stage.lower()]=sorted(rows,key=lambda r:list(expected).index(r['id']))
    paths=list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+d+'/'+family+'.json'for d in ['fixtures','observations'])
        paths.extend(str(p.relative_to(ROOT))for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return dict(native_engine_version=version,model_epsilon=1e-6,captured_workflows=len(reference),native_workflows=len(qualified),
        command_cases=len(stages['command']),getter_cases=len(stages['getter']),maximum_error=maximum,
        fresh_defaults=dict(select_by='Distance',distance=0.,angle=0.,split_method='ShortestDiagonal'),
        full_native_parity=False,performance_qualified=False,excluded=EXCLUDED,
        failed_import_requested_workflows=6,failed_import_qualified_workflows=0,**stages,
        limits=[
            'Native scripted command inputs and exact saved source archives are independent of output geometry. Replays compare raw mesh records, stored normals/UV/colors, surviving n-gons, identity, attributes, groups, object table order, selection and document Undo/Redo.',
            'Fresh native scripted defaults are SelectBy=Distance, Distance=0, Angle=0 and ShortestDiagonal. Bare SelectBy toggles Distance and Angle. Active numeric thresholds use a separate response. Native area choices are MinimumArea and MaximumArea; legacy MinimizeArea/MaximizeArea and Mode aliases remain local extensions.',
            'Accepted options persist across documents and Undo/Redo within a registry. Cancellation preserves accepted settings. Success clears selection, skips non-mesh sources, moves processed meshes to the object table end and records Undo even on unchanged meshes. Input geometry and tolerance are pinned while local options are staged.',
            'The application shares one staged mesh conversion getter with QuadrangulateMesh. Invalid numeric updates are atomic; cancellation preserves redo. Native saved-script postselection uses All, not physical mouse input. Select Quad threshold sampling, dialogs/live previews, spinner controls, restricted objects, grips, multi-mesh processing order, extreme values and comparative performance remain unqualified.',
            'Five discovery workflows are explicitly excluded because Rhino rejected an option, invoked a different measurement command or accepted before the intended options. Corrected postselection captures omit the extra Enter after All. The local combined criterion uses the separately qualified SDK path; no native scripted combined-mode parity is claimed. A failed IronPython import retains its six requested workflows, host log and frozen producers with zero qualified geometry.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    result=summarize('\n'.join(p.read_text()for p in args.logs))
    (ROOT/'docs/nonplanar-command-qualification.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
if __name__=='__main__':main()
