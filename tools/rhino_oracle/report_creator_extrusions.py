"""Qualify typed creators, signed paths, 3DM round-trips and ToSubD outputs."""
from .qualification_sources import with_shared_sources
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_tosubd_commands import ROOT, SOURCES as COMMAND_SOURCES
from .report_subdivide import subd_check
from .report_subd_box import error
HERE=Path(__file__).resolve().parent
SOURCES=list(dict.fromkeys(COMMAND_SOURCES+[
    'crates/viboceros-command/src/cylinder.rs',
    'crates/viboceros-command/src/cylinder/tests.rs',
    'crates/viboceros-command/src/plane_primitives.rs',
    'crates/viboceros-command/src/offset_surface.rs',
    'crates/viboceros-command/src/extrude_curve.rs',
    'crates/viboceros-command/src/extrude_curve/tests.rs',
    'crates/viboceros-command/src/extrude_curve/tests/legacy.rs',
    'crates/viboceros-document/src/geometry.rs',
    'crates/viboceros-geometry/src/brep/extrusion.rs',
    'crates/viboceros-oracle/src/curve_native.rs',
    'src/app/tests/affine_preview.rs',
    'docs/creator-extrusion-direction-regression.json',
    'crates/viboceros-command/tests/creator_extrusions_native.rs',
    'tools/rhino_oracle/creator_extrusions_probe.py',
    'tools/rhino_oracle/generate_creator_extrusions.py',
    'tools/rhino_oracle/report_creator_extrusions.py',
    'tools/rhino_oracle/test_creator_extrusions.py',
    'tools/rhino_oracle/fixtures/creator_extrusions.json',
    'tools/rhino_oracle/observations/creator_extrusions.json',
    'tools/rhino_oracle/fixtures/creator_extrusions_macro_diagnostic.json',
    'tools/rhino_oracle/observations/creator_extrusions_macro_diagnostic.json',
]))
SOURCES = with_shared_sources(SOURCES)


def observations():
    query=json.loads((HERE/'fixtures/creator_extrusions.json').read_text())
    native=json.loads((HERE/'observations/creator_extrusions.json').read_text())
    if len(native['results'])!=42 or [r['id']for r in native['results']]!=[o['id']for o in query['operations']]:
        raise ValueError('incomplete or reordered creator observations')
    result={}
    for row in native['results']:
        value=row['value'];outputs=[o for o in value['created']if not o['source']]
        events=[e for e in value['events']if e['name']==value['input']['command']]
        if row['id']in result or value['success']is not True or len(outputs)!=1 or len(events)!=1 or events[0]['result']!='Success' or events[0]['objects']!=value['created'] or 'Unknown command:'in value['history']:
            raise ValueError('invalid native creator lifecycle')
        result[row['id']]=value
    return result,native['engine_version']


def summarize(text,backend="native-smlib"):
    if backend not in ("native-smlib","portable"):raise ValueError("unknown creator modeling backend")
    values,engine=observations()
    rows=[json.loads(s)for s in re.findall(r'^CREATOR_EXTRUSIONS (.+)$',text,re.M)]
    found={r['id']:r for r in rows}
    if len(rows)!=42 or set(found)!=set(values):raise ValueError('missing, duplicate or unrelated creator comparisons')
    surface_error=bounds_error=control_error=facet_error=0.
    typed=conversions=0
    for key,value in values.items():
        row=found[key];output=next(o for o in value['created']if not o['source']);expected_type='extrusion'in output
        if (type(row.get('typed_extrusion'))is not bool or row['typed_extrusion']!=expected_type
            or any(row.get(k)is not True for k in ['source_unchanged','history','roundtrip'])
            or type(row.get('cyclic_box_profile'))is not bool or row['cyclic_box_profile']!=(value['input']['command']=='Box')):
            raise ValueError('uncertified creator type, history or interchange')
        surface_error=max(surface_error,error(row.get('maximum_surface_control_error')))
        bounds_error=max(bounds_error,error(row.get('maximum_bounds_error')))
        typed+=expected_type
        if value['tosubd_success']is True:
            targets=[o['subd']for o in value['after']if 'subd'in o]
            if len(targets)!=1:raise ValueError('ambiguous native creator conversion')
            c,f=subd_check(row['conversion'],targets[0]);control_error=max(control_error,c);facet_error=max(facet_error,f);conversions+=1
        elif row.get('conversion')is not None:raise ValueError('unexpected creator conversion')
    if typed!=39 or conversions!=40:raise ValueError('creator conversion coverage changed')
    return dict(model_epsilon=1e-6,native_engine_version=engine,creator_cases=42,modeling_backend=backend,
        typed_extrusion_creators=39,explicit_brep_creators=3,tosubd_comparisons=40,
        maximum_surface_control_error=surface_error,maximum_bounds_error=bounds_error,
        maximum_control_or_limit_point_error=control_error,maximum_sampled_surface_error=facet_error,
        full_native_parity=False,performance_qualified=False,cases=[found[k]for k in values],
        limits=[
            'Fixed Box, Cylinder and ExtrudeCrv creators cover signed heights, BothSides, caps, line and kink profiles, a tilted box, nonplanar and oblique fallbacks, geometry-preserving undo/redo, native 3DM type retention and whole ToSubD conversion.',
            'Box wall controls are matched geometrically under cyclic profile seams. Exact Box parameter origins, seam choices and canonical face ordering remain unqualified. Other creator surfaces compare full control grids, weights and knot intervals; explicit oblique walls allow axis reversals/transposition and normalized knot intervals.',
            'Visible bounds are compared against the captured native wall surfaces. Trimmed-cap support rectangles can differ; cap parameterization and all native attributes are not qualified by this record.',
            'The initial failed Cylinder option-phase capture is archived as a diagnostic and excluded. The corrected 42 workflows use the observed Solid option before the center prompt.',
            'Native cross-command preferences, physical pointer getters, arbitrary profile topology, continuous surface error and comparative performance remain unqualified.',
        ],sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in SOURCES})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path);parser.add_argument('--portable',action='store_true');args=parser.parse_args()
    result=summarize('\n'.join(p.read_text()for p in args.logs),'portable'if args.portable else'native-smlib')
    (ROOT/('docs/creator-extrusion-portable-qualification.json'if args.portable else'docs/creator-extrusion-qualification.json')).write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
