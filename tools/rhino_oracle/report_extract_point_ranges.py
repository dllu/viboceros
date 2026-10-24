"""Compare measured ExtractPt temporary inputs to immutable native scenes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from .extract_points_cases import FAMILIES as ALL_FAMILIES
from .report_extract_points import ROOT, native, canonical, compare
from .qualification_sources import with_shared_sources

FAMILIES={name:ALL_FAMILIES[name] for name in ['ranges','ranges_grips','ranges_owners']}
SOURCES=with_shared_sources([
    'crates/viboceros-command/src/lib.rs','src/app.rs',
    'tools/rhino_oracle/report_extract_point_ranges.py','tools/rhino_oracle/test_extract_point_ranges.py',
    'tools/rhino_oracle/extract_points_cases.py','tools/rhino_oracle/extract_points_probe.py',
    'tools/rhino_oracle/generate_extract_points.py','tools/rhino_oracle/report_extract_points.py',
    'docs/commands/extract-point-ranges.md',
])

def native_ranges():
    version,rows,paths=native(FAMILIES,qualified=FAMILIES)
    for identifier,value in rows.items():
        trace=value['getter']
        recipe=value['input']['ranges']
        if (len(trace['parameters'])!=len(recipe) or len(trace['points'])!=len(recipe) or
            trace['script'].count(' _SubCrv ')!=len(recipe) or
            'Unknown command:' in value['history']):
            raise ValueError('ExtractPt native temporary input receipt differs')
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
    return dict(scope='Standalone temporary curve portions, mixed whole objects and grips, attributes, selection and Undo/Redo',
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
