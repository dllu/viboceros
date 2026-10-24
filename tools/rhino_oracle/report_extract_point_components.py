"""Audit fixed native component requests and compare measured command/getter/edge states."""
import argparse
import hashlib
import json
from pathlib import Path
from .extract_points_cases import FAMILIES
from .report_extract_points import ROOT, canonical, compare, native
from .qualification_sources import with_shared_sources

FAMILIES = {k:v for k,v in FAMILIES.items() if k.startswith('component')}
SOURCES = with_shared_sources([
    'crates/viboceros-command/src/lib.rs', 'src/app.rs',
    'crates/viboceros-geometry/src/nurbs.rs','crates/viboceros-geometry/src/subd.rs',
    'tools/rhino_oracle/client.py','tools/rhino_oracle/component_picking.py','tools/rhino_oracle/hole_picking.py',
    'tools/rhino_oracle/group_picking.py','tools/rhino_oracle/managed_component_input.py',
    'crates/viboceros-geometry/src/subd/edges.rs', 'crates/viboceros-geometry/src/subd/edges/curves.rs',
    'crates/viboceros-opennurbs/native/subd_edges.h', 'crates/viboceros-opennurbs/src/subd/edges.rs',
    'tools/rhino_oracle/test_component_picking.py',
    'tools/rhino_oracle/report_extract_point_components.py', 'tools/rhino_oracle/test_extract_point_components.py',
    'tools/rhino_oracle/report_extract_points.py', 'tools/rhino_oracle/extract_points_cases.py',
    'tools/rhino_oracle/extract_points_component_cases.py','tools/rhino_oracle/extract_points_component_input.py',
    'tools/rhino_oracle/extract_points_getter.py','tools/rhino_oracle/extract_points_geometry.py','tools/rhino_oracle/extract_points_probe.py',
    'tools/rhino_oracle/generate_extract_points.py','docs/commands/extract-point-components.md',
])


def states(rows, components):
    rows = canonical(rows)
    for row in rows:
        row = row.copy()
        if components:
            row['components'] = [list(p) for p in sorted({tuple(p) for p in row['components']})]
        else:
            row.pop('components')
        yield row


def measured(log, prefix, keys, key):
    rows = [json.loads(line.split(prefix+' ',1)[1]) for line in log.splitlines() if prefix+' ' in line]
    actual = [key(row) for row in rows]
    if len(actual) != len(set(actual)) or set(actual) != keys:
        raise ValueError('ExtractPt component measurement coverage differs')
    return sorted(rows,key=key)


def summarize(command_log, getter_log, edge_log):
    version, expected, paths = native(FAMILIES,components=True)
    paths.update(SOURCES)
    commands = measured(command_log,'EXTRACT_POINTS_COMPONENT_COMMAND',set(expected),lambda r:r['id'])
    getters = measured(getter_log,'EXTRACT_POINTS_COMPONENT_GETTER',
        {(name,value['_path']) for name,value in expected.items()},lambda r:(r['id'],r['path']))
    for components,rows in [(False,commands),(True,getters)]:
        for row in rows:
            if row['success'] is not True or row['source_storage_unchanged'] is not True:
                raise ValueError('ExtractPt component status or source storage differs')
            row['maximum_deviation'] = max(compare(row[stage],list(states(expected[row['id']][stage],components))) for stage in ['after','undo','redo'])
    profiles = {(name,i):profile for name,value in expected.items() if isinstance(value['curve_profile'],list)
                for i,profile in enumerate(value['curve_profile'])}
    edges = measured(edge_log,'EXTRACT_POINTS_EDGE_FORM',set(profiles),lambda r:(r['id'],r['edge']))
    for row in edges:
        wanted = {k:v for k,v in profiles[row['id'],row['edge']].items() if k != 'greville'}
        row['maximum_deviation'] = compare({k:v for k,v in row.items() if k not in ['id','edge']},wanted)
    return dict(scope='B-rep/Mesh/SubD components, explicit surface/control-net locations, attributes, transient selection and history; flat rendering is outside this scope',
        native_version=version,epsilon=1e-7,captured=sum(len(c) for c in FAMILIES.values()),
        command_replayed=len(commands),getter_replayed=len(getters),edge_forms_replayed=len(edges),
        maximum_deviation=max(r['maximum_deviation'] for r in commands+getters+edges),
        commands=commands,getters=getters,edge_forms=edges,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['command_log','getter_log','edge_log']:parser.add_argument(name,type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/extract-point-components-qualification.json')
    args=parser.parse_args()
    record=summarize(args.command_log.read_text(),args.getter_log.read_text(),args.edge_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('ExtractPt components: {} commands, {} getters, {} edge forms; maximum deviation {}'.format(
        record['command_replayed'],record['getter_replayed'],record['edge_forms_replayed'],record['maximum_deviation']))


if __name__ == '__main__':main()
