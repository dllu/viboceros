"""Compare whole-SubD extraction to immutable smooth/flat Rhino scenes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from .extract_points_cases import FAMILIES as ALL_FAMILIES
from .qualification_sources import with_shared_sources
from .report_extract_points import ROOT, native, measurements

FAMILIES={name:ALL_FAMILIES[name] for name in ['whole_subd_picked','whole_subd_final']}
DIAGNOSTICS={name:ALL_FAMILIES[name] for name in ['whole_subd','whole_subd_mixed','whole_subd_verified']}
SOURCES=with_shared_sources([
    'crates/viboceros-command/src/lib.rs','crates/viboceros-document/src/lib.rs','src/app.rs',
    'crates/viboceros-geometry/src/subd.rs','crates/viboceros-opennurbs/native/subd.h',
    'crates/viboceros-opennurbs/native/subd.cpp','crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-command/tests/extract_points_subd_native.rs',
    'tools/rhino_oracle/report_extract_points_subd.py','tools/rhino_oracle/test_extract_points_subd.py',
    'docs/commands/extract-points.md','docs/commands/extract-point-components.md',
])


def native_subds():
    version,rows,paths=native(FAMILIES,qualified=FAMILIES)
    diagnostic_version,_,diagnostic_paths=native(DIAGNOSTICS,qualified={})
    if diagnostic_version!=version:raise ValueError('Whole SubD native versions differ')
    paths.update(diagnostic_paths)
    for row in rows.values():
        recipe,witness=row['input'],row['subd_witness']
        row['_path']='picked' if recipe.get('getter') else 'preselected'
        options=['_OutputLayer=_'+recipe['layer'],'_Output=_'+recipe['output']]
        if recipe.get('output_first'):options.reverse()
        if row['macro']!='_-ExtractPt '+' '.join(options)+('' if recipe.get('getter') else ' _Enter'):
            raise ValueError('Whole SubD command option witness differs')
        if (type(witness.get('control_net')) is not bool or witness['control_net']!=recipe['flat'] or
                len(witness['sources'])!=1 or witness['sources'][0]['source']!=0 or
                'Unknown command:' in row['history']):
            raise ValueError('Whole SubD appearance or source witness differs')
        vertices=witness['sources'][0]['vertices']
        ids=[v['id'] for v in vertices]
        if (not vertices or len(ids)!=len(set(ids)) or
                any(type(i) is not int or i<=0 for i in ids) or
                any(len(v[field])!=3 or any(type(x) not in (float,int) or not math.isfinite(x) for x in v[field])
                    for v in vertices for field in ['control','surface'])):
            raise ValueError('Whole SubD vertex witness differs')
        points=[p for o in row['after'] if o['source'] is None for p in o['points']]
        outputs=[o for o in row['after'] if o['source'] is None]
        if (any(o['cloud']!=(recipe['output']=='PointCloud') for o in outputs) or
                (recipe['output']=='PointCloud' and len(outputs)!=1) or
                (recipe['layer']=='Current' and any(o['layer']!='Extract current' for o in outputs))):
            raise ValueError('Whole SubD output option was not applied')
        expected=[v['control' if recipe['flat'] else 'surface'] for v in vertices]
        offsets=range(len(points)-len(expected)+1) if len(recipe['sources'])>1 else [0]
        if (len(recipe['sources'])==1 and len(points)!=len(expected)) or not any(
                all(math.dist(a,b)<=1e-7 for a,b in zip(points[offset:offset+len(expected)],expected))
                for offset in offsets):
            raise ValueError('Whole SubD output disagrees with native vertex witness')
        if recipe.get('getter'):
            trace=row['getter']
            selection=' '.join('_SelID '+row['before'][i]['id'] for i in recipe['whole'])
            if (trace.get('selection')!=selection or
                    trace.get('visible_grips')!=sum(len(s['grips']) for s in recipe['sources'])):
                raise ValueError('Whole SubD command-first selection witness differs')
    return version,rows,paths


def summarize(command_log,getter_log):
    version,rows,paths=native_subds()
    commands=measurements(command_log,'EXTRACT_POINTS_SUBD_COMMAND',rows)
    getters=measurements(getter_log,'EXTRACT_POINTS_SUBD_GETTER',rows)
    return dict(scope='Whole SubD surface/control-net vertices, preselected and command-first inputs, mixed sources, attributes and Undo/Redo',
        native_version=version,epsilon=1e-7,captured=len(rows),command_replayed=len(commands),getter_replayed=len(getters),
        diagnostic_captures=sum(len(c) for c in DIAGNOSTICS.values()),
        maximum_deviation=max(r['maximum_deviation'] for r in commands+getters),commands=commands,getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('getter_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/extract-points-subd-qualification.json')
    args=parser.parse_args()
    record=summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('Whole SubD ExtractPt: {} commands, {} getters; maximum deviation {}'.format(
        record['command_replayed'],record['getter_replayed'],record['maximum_deviation']))


if __name__=='__main__':main()
