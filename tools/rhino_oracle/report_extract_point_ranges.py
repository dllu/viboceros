"""Compare measured ExtractPt temporary inputs to immutable native scenes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from .extract_points_cases import FAMILIES as ALL_FAMILIES
from .report_extract_points import ROOT, native, canonical, compare
from .qualification_sources import with_shared_sources

FAMILIES={name:ALL_FAMILIES[name] for name in ['ranges','ranges_grips','ranges_owners','ranges_edges','ranges_edge_forms','ranges_edge_numeric_verified','ranges_order','ranges_order_grips','ranges_order_edges','ranges_order_displays']}
SOURCES=with_shared_sources([
    'crates/viboceros-command/src/lib.rs','src/app.rs',
    'tools/rhino_oracle/report_extract_point_ranges.py','tools/rhino_oracle/test_extract_point_ranges.py',
    'tools/rhino_oracle/extract_points_cases.py','tools/rhino_oracle/extract_points_probe.py',
    'tools/rhino_oracle/generate_extract_points.py','tools/rhino_oracle/report_extract_points.py',
    'docs/commands/extract-point-ranges.md',
    'crates/viboceros-command/src/curve_reference.rs',
    'crates/viboceros-command/src/polycurve_tests.rs',
    'crates/viboceros-geometry/src/brep.rs','crates/viboceros-geometry/src/interop/mod.rs',
    'crates/viboceros-geometry/src/interop/tests.rs',
    'crates/viboceros-io/src/three_dm/nested_codec_tests.rs',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'src/app/subcurve_direction.rs','src/app/subcurve_preview.rs','src/app/subcurve_prompt.rs',
])

def native_ranges():
    version,rows,paths=native(FAMILIES,qualified=FAMILIES)
    for identifier,value in rows.items():
        trace=value['getter']
        edge_ranges=value['input'].get('edge_ranges')
        recipe=edge_ranges or value['input']['ranges']
        inputs=value['input'].get('input_order')
        if inputs:
            if (trace.get('inputs')!=inputs or trace.get('world_coordinates') is not True or
                sorted(i for kind,i in inputs if kind=='range')!=list(range(len(recipe))) or
                [i for kind,i in inputs if kind=='whole']!=value['input']['whole'] or
                any(kind not in ['whole','range'] or type(i) is not int or i<0 for kind,i in inputs)):
                raise ValueError('ExtractPt native mixed input order differs')
            position=0
            for kind,index in inputs:
                source=index if kind=='whole' else recipe[index][0]
                marker=(' _SubCrv _Pause' if kind=='range' and edge_ranges else
                    (' _SubCrv' if kind=='range' else '')+' _SelID '+value['before'][source]['id'])
                found=trace['script'].find(marker,position)
                if found<0:raise ValueError('ExtractPt native input macro order differs')
                position=found+len(marker)
        if (len(trace['parameters'])!=len(recipe) or len(trace['points'])!=len(recipe) or
            trace['script'].count(' _SubCrv ')!=len(recipe) or
            'Unknown command:' in value['history']):
            raise ValueError('ExtractPt native temporary input receipt differs')
        if edge_ranges and (trace.get('edges')!=[r[1] for r in edge_ranges] or
                trace.get('world_coordinates') is not True or trace.get('acknowledged') is not True or
                trace.get('frustum_verified') is not True or type(trace.get('menu')) is not bool or
                trace.get('marker')!='@component-click:{}:0:sub'.format(identifier) or
                (not inputs and not value['input'].get('range_whole_first')) or trace['script'].count(' w')!=2):
            raise ValueError('ExtractPt native edge input witness differs')
        if identifier.startswith('range_form_'):
            expected_type='NurbsCurve' if value['input']['sources'][0]['kind']=='brep_nurbs_cylinder' else 'ArcCurve'
            if trace.get('curve_type')!=expected_type:
                raise ValueError('ExtractPt native edge curve representation differs')
        if edge_ranges:
            produced=[p for o in value['after'] if o['source'] is None for p in o['points']]
            # These owned scenes contain zero or one whole LineCurve and one
            # temporary edge. Verify the endpoint witness against actual
            # ExtractPt output, beyond the read-only frustum check.
            sequence=inputs or ([['whole',i] for i in value['input']['whole']]+[['range',0]])
            split=sequence.index(['range',0])
            offset=2*sum(kind=='whole' for kind,_ in sequence[:split])
            trailing=2*sum(kind=='whole' for kind,_ in sequence[split+1:])
            if len(produced)<=offset or any(math.dist(a,b)>1e-7 for a,b in
                    zip([produced[offset],produced[len(produced)-trailing-1]],trace['points'][0][1:])):
                raise ValueError('ExtractPt native edge output endpoints differ')
        if value['input'].get('edge_length'):
            if (type(trace.get('numeric_length')) not in (float,int) or trace['numeric_length']<=0 or
                not math.isfinite(trace['numeric_length']) or len(trace.get('input_points',[]))!=1 or
                trace['input_points'][0][0]!=edge_ranges[0][0] or
                trace['input_points'][0][2]!=trace.get('confirmation') or
                any(len(p)!=3 or not all(math.isfinite(x) for x in p) for p in trace['input_points'][0][1:])):
                raise ValueError('ExtractPt native numeric edge witness differs')
        for expected,parameters,points in zip(recipe,trace['parameters'],trace['points']):
            if (len(parameters)!=3 or len(points)!=3 or
                parameters[0]!=expected[0] or points[0]!=expected[0] or
                any(type(p) not in (float,int) or not math.isfinite(p) for p in parameters[1:]) or
                any(len(p)!=3 or any(type(x) not in (float,int) or not math.isfinite(x) for x in p) for p in points[1:])):
                raise ValueError('ExtractPt native temporary input parameters differ')
    return version,rows,paths

def measured(log,prefix,expected,key):
    rows=[json.loads(line.split(prefix+' ',1)[1]) for line in log.splitlines() if prefix+' ' in line]
    keys=[key(r) for r in rows]
    if len(keys)!=len(set(keys)) or set(keys)!=expected:
        raise ValueError('ExtractPt temporary measurement coverage differs')
    return sorted(rows,key=key)

def summarize(command_log,getter_log):
    version,expected,paths=native_ranges()
    commands=measured(command_log,'EXTRACT_POINTS_RANGE_COMMAND',set(expected),lambda r:r['id'])
    getters=measured(getter_log,'EXTRACT_POINTS_RANGE_GETTER',
        {(identifier,path) for identifier in expected for path in ['inline','promoted']},lambda r:(r['id'],r['path']))
    for row in commands+getters:
        if row['success'] is not True or row['source_storage_unchanged'] is not True:
            raise ValueError('ExtractPt temporary status or storage differs')
        row['maximum_deviation']=max(compare(row[stage],canonical(expected[row['id']][stage])) for stage in ['after','undo','redo'])
    return dict(scope='Temporary standalone curves and B-rep edges, mixed whole objects and grips, attributes, selection and Undo/Redo',
        native_version=version,epsilon=1e-7,captured=len(expected),command_replayed=len(commands),getter_replayed=len(getters),
        maximum_deviation=max(r['maximum_deviation'] for r in commands+getters),commands=commands,getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('getter_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/extract-point-ranges-qualification.json')
    args=parser.parse_args();record=summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('ExtractPt portions: {} commands, {} getters; maximum deviation {}'.format(record['command_replayed'],record['getter_replayed'],record['maximum_deviation']))

if __name__=='__main__':main()
