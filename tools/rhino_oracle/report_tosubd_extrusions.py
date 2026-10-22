"""Qualify extrusion and direct surface-SDK conversion, with complete fixed coverage."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subdivide import subd_check
from .report_extrusions import SOURCES as INTEROP_SOURCES
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
FAMILIES=[('tosubd_extrusions',40),('tosubd_pipes',9),('tosubd_conversion_edges',18)]
SOURCES=list(dict.fromkeys(INTEROP_SOURCES+[
    'crates/viboceros-opennurbs/src/extrusion/surface.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/src/subd/extrusion_conversion.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/features.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/fitting.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/axis.rs',
    'crates/viboceros-command/tests/tosubd_extrusions_native.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'tools/rhino_oracle/report_tosubd_extrusions.py',
    'tools/rhino_oracle/test_tosubd_extrusions.py',
    'tools/rhino_oracle/creator_types_probe.py','tools/rhino_oracle/generate_creator_types.py',
    'tools/rhino_oracle/fixtures/creator_types.json','tools/rhino_oracle/observations/creator_types.json',
]+[f'tools/rhino_oracle/{folder}/{family}.json' for family,_ in FAMILIES for folder in ['fixtures','observations']]))


def observations():
    values,engines={},set()
    for family,count in FAMILIES:
        query=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        data=json.loads((HERE/'observations'/(family+'.json')).read_text())
        if len(data['results'])!=count or [r['id']for r in data['results']]!=[o['id']for o in query['operations']]:
            raise ValueError('missing or reordered extrusion conversion captures')
        engines.add(data['engine_version'])
        for row in data['results']:
            if row['id']in values:raise ValueError('duplicate extrusion conversion case')
            values[row['id']]=row['value']
    if len(engines)!=1:raise ValueError('mixed native conversion engines')
    return values,engines.pop()


def summarize(text):
    values,engine=observations()
    targets={'extrusion_kernel':{},'surface_sdk_kernel':{}}
    for key,value in values.items():
        if value['input']['sources'][0]['kind']=='extrusion' and 'tube_'not in key and key!='tosubd_pipe_profile_frame_diagnostic':
            if value['success']is not True:raise ValueError('native extrusion conversion failed')
            outputs=[o['subd']for o in value['after_script']if 'subd'in o]
            if len(outputs)!=1:raise ValueError('ambiguous native extrusion output')
            targets['extrusion_kernel'][key]=outputs[0]
        if value['sdk_inputs']:
            if len(value['sdk_inputs'])!=1 or value['sdk_inputs'][0]['subd']is None:raise ValueError('invalid direct SDK evidence')
            targets['surface_sdk_kernel'][key]=value['sdk_inputs'][0]['subd']
    if [len(t)for t in targets.values()]!=[54,8]:raise ValueError('conversion coverage changed')
    stages={};control_error=facet_error=0.
    for stage,wanted in targets.items():
        rows=[json.loads(line)for line in re.findall('^TOSUBD_'+stage.upper()+r' (.+)$',text,re.M)]
        found={r['id']:r for r in rows}
        if len(rows)!=len(wanted) or set(found)!=set(wanted):raise ValueError('missing, duplicate or unrelated '+stage)
        for key,target in wanted.items():
            row=found[key]
            if row.get('source_unchanged')is not True:raise ValueError('uncertified immutable source')
            c,f=subd_check(row['check'],target);control_error=max(control_error,c);facet_error=max(facet_error,f)
        stages[stage]=[found[k]for k in wanted]
    creators=json.loads((HERE/'observations/creator_types.json').read_text())
    if len(creators['results'])!=8 or creators['engine_version']!=engine:raise ValueError('missing creator discovery')
    for row in creators['results']:
        value=row['value'];outputs=[o for o in value['created']if not o['source']]
        if value['success']is not True or value['tosubd_success']is not True or len(outputs)!=1 or outputs[0]['type']!='Extrusion':raise ValueError('native creator representation changed')
    files=[f'tools/rhino_oracle/fixtures/extrusions/{key}.3dm'for key,value in values.items()if value['source_files']]
    return dict(model_epsilon=1e-6,native_engine_version=engine,captured_workflows=67,
        extrusion_kernel_comparisons=54,direct_surface_sdk_comparisons=8,creator_type_observations=8,
        maximum_control_or_limit_point_error=control_error,maximum_sampled_surface_error=facet_error,
        command_qualified=False,getter_qualified=False,full_native_parity=False,performance_qualified=False,
        stages=stages,limits=[
            'Kernel conversion only. ToSubD command/getter and document lifecycle are qualified separately in tosubd-command-qualification.json; source creator representation updates remain pending.',
            'Fixed native comparisons cover controls, topology, tags, vertex limit stations and corresponding density-two facets within 1e-6; arbitrary profiles and continuous surface error are not certified.',
            'Complete-profile extrusion NURBS conversion follows public OpenNURBS joining. Closed complete profiles receive caps only when both ends are capped. Single-cap inputs retain only their walls; multiple profiles can produce an open joined-profile ribbon, matching the captures.',
            'Direct SDK cases distinguish degree-one and degree-two kink behavior before Rhino document insertion splits them into multi-face B-reps. These eight cases are excluded from constructive command totals.',
            'Eight public creator observations show native default Box, Cylinder and planar ExtrudeCrv produce extrusions. Their Viboceros creator updates remain pending; discovery is not command qualification.',
            'Five misplaced-inner-profile workflows are retained as diagnostics and excluded from conversion comparisons; their invalid native B-rep form was recorded separately.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in dict.fromkeys(SOURCES+files)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);args=parser.parse_args()
    result=summarize('\n'.join(p.read_text()for p in args.logs))
    (ROOT/'docs/tosubd-extrusion-qualification.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')

if __name__=='__main__':main()
