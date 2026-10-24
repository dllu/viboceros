"""Recompute native grip extraction from raw Rust document snapshots."""
import argparse
import base64
import gzip
import hashlib
import json
import math
from pathlib import Path
from .extract_points_cases import FAMILIES, request
from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCES = with_shared_sources([
    'crates/viboceros-command/src/lib.rs', 'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-document/src/lib.rs', 'src/app.rs',
    'crates/viboceros-command/tests/extract_points_native.rs',
    'crates/viboceros-command/tests/support/extract_points.rs',
    'tools/rhino_oracle/extract_points_cases.py', 'tools/rhino_oracle/extract_points_probe.py',
    'tools/rhino_oracle/generate_extract_points.py', 'tools/rhino_oracle/report_extract_points.py',
    'tools/rhino_oracle/test_extract_points.py', 'docs/commands/extract-points.md',
])


def native():
    rows, paths, versions = {}, set(SOURCES), set()
    for family, cases in FAMILIES.items():
        name = 'extract_points_'+family
        query, observation = HERE/'fixtures'/(name+'.json'), HERE/'observations'/(name+'.json')
        data = json.loads(observation.read_text())
        if json.loads(query.read_text()) != request(family):
            raise ValueError('ExtractPt fixed native request changed')
        if hashlib.sha256(query.read_bytes()).hexdigest() != data['provenance']['request_sha256']:
            raise ValueError('ExtractPt native request digest changed')
        paths.update([str(query.relative_to(ROOT)), str(observation.relative_to(ROOT))])
        for source, digest in data['provenance']['sources'].items():
            path = HERE/'observations/source_snapshots'/(digest+'.gz')
            if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest() != digest:
                raise ValueError('ExtractPt original native producer changed: '+source)
            paths.add(str(path.relative_to(ROOT)))
        if [r['id'] for r in data['results']] != list(cases):
            raise ValueError('ExtractPt native coverage changed')
        versions.add(data['engine_version'])
        for row in data['results']:
            value, identifier = row['value'], row['id']
            if value['input'] != cases[identifier] or value['success'] is not True:
                raise ValueError('ExtractPt native inputs or status changed')
            path = HERE/'fixtures'/name/(identifier+'.3dm')
            if path.read_bytes() != base64.b64decode(value['source_3dm']):
                raise ValueError('ExtractPt original source file changed')
            paths.add(str(path.relative_to(ROOT)))
            sources = {o['source']:(o['id'],o['crc']) for o in value['before']}
            for stage in ['after','undo','redo']:
                if sources != {o['source']:(o['id'],o['crc']) for o in value[stage] if o['source'] is not None}:
                    raise ValueError('ExtractPt native source identity or geometry changed')
            if family in ['verified','metadata']:
                if value['getter'] is None or value['getter']['visible_grips'] != sum(len(s['grips']) for s in value['input']['sources']):
                    raise ValueError('ExtractPt native getter coverage changed')
            if family in ['verified','metadata'] or (family == 'grips' and value['input']['output'] == 'Points'):
                value['_path'] = 'preselected' if family == 'grips' else 'picked'
                rows[identifier] = value
    if len(versions) != 1 or len(rows) != 63:
        raise ValueError('ExtractPt capture domain changed')
    return versions.pop(), rows, paths


def canonical(rows):
    return [{k:v for k,v in row.items() if k not in ['id','crc']} for row in rows]


def compare(actual, expected):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or set(actual) != set(expected):
            raise ValueError('ExtractPt measured object fields differ')
        return max((compare(actual[k],v) for k,v in expected.items()), default=0.)
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            raise ValueError('ExtractPt measured object or point counts differ')
        return max((compare(a,b) for a,b in zip(actual,expected)), default=0.)
    if type(expected) is float:
        if type(actual) not in (float,int) or not math.isfinite(actual) or abs(actual-expected) > 1e-7:
            raise ValueError('ExtractPt measured location differs')
        return abs(actual-expected)
    if type(actual) is not type(expected) or actual != expected:
        raise ValueError('ExtractPt measured metadata or selection differs')
    return 0.


def measurements(log, prefix, expected, getter=False):
    rows = []
    for line in log.splitlines():
        if prefix+' ' not in line:
            continue
        rows.append(json.loads(line.split(prefix+' ',1)[1]))
    wanted = {(name,value['_path']) for name,value in expected.items()}
    keys = [(r['id'], r['path']) for r in rows]
    if len(keys) != len(set(keys)) or set(keys) != wanted:
        raise ValueError('ExtractPt Rust replay coverage differs')
    for row in rows:
        if row['success'] is not True or row['source_storage_unchanged'] is not True:
            raise ValueError('ExtractPt Rust status or ownership differs')
        row['maximum_deviation'] = max(compare(row[s], canonical(expected[row['id']][s])) for s in ['after','undo','redo'])
    return sorted(rows,key=lambda r:(r['id'],r.get('path','command')))


def summarize(command_log, getter_log):
    version, expected, paths = native()
    commands = measurements(command_log,'EXTRACT_POINTS_GRIPS_COMMAND',expected)
    getters = measurements(getter_log,'EXTRACT_POINTS_GRIPS_GETTER',expected,True)
    return dict(scope='Selected control/edit grips with mixed whole objects, attributes, groups, visibility and Undo/Redo',
        native_version=version, epsilon=1e-7, captured=sum(len(c) for c in FAMILIES.values()),
        command_replayed=len(commands), getter_replayed=len(getters),
        maximum_deviation=max(r['maximum_deviation'] for r in commands+getters),
        diagnostic_captures=sum(len(c) for c in FAMILIES.values())-len(expected), commands=commands, getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/extract-points-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('ExtractPt: {} commands, {} getters; maximum deviation {}'.format(
        record['command_replayed'],record['getter_replayed'],record['maximum_deviation']))


if __name__ == '__main__':
    main()
