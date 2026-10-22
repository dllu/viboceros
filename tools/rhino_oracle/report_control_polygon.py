"""Qualify whole-object control polygon commands and their interactive getters."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT/'tools/rhino_oracle'
FAMILIES = ['control_polygon', 'control_polygon_options', 'control_polygon_priority']
SOURCES = with_shared_sources([
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/src/extract_control_polygon.rs',
    'crates/viboceros-command/tests/extract_control_polygon_native.rs',
    'crates/viboceros-command/tests/support/control_polygon.rs',
    'crates/viboceros-document/src/object_copy.rs',
    'crates/viboceros-document/src/lib.rs',
    'crates/viboceros-document/src/history.rs',
    'crates/viboceros-document/src/object_layer.rs',
    'crates/viboceros-document/src/selection.rs',
    'crates/viboceros-geometry/src/nurbs_surface.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'src/app/object_selection.rs',
    'src/app.rs',
    'src/app/object_selection/control_polygon_tests.rs',
    'tools/rhino_oracle/report_control_polygon.py',
    'tools/rhino_oracle/test_control_polygon.py',
])


def expected():
    trusted, diagnostics = {}, []
    for family in FAMILIES:
        for row in json.loads((HERE/'observations'/(family+'.json')).read_text())['results']:
            if 'Unknown' in row['value']['history']:
                diagnostics.append(dict(family=family, id=row['id']))
            else:
                trusted[(family, row['id'])] = row['value']['success']
    return trusted, diagnostics


def summarize(command_log, ui_log):
    wanted, diagnostics = expected()
    streams = {}
    for kind, marker, log in [('command', 'CONTROL_POLYGON_NATIVE', command_log),
                              ('ui', 'CONTROL_POLYGON_UI', ui_log)]:
        rows = [json.loads(s) for s in re.findall(r'^'+marker+r' (.+)$', log, re.M)]
        seen = set()
        for row in rows:
            fields = {'family', 'id', 'success', 'maximum_error', 'metadata_exact',
                      'selection_exact', 'history_exact', 'qualified'}
            if kind == 'command':
                fields.add('geometry_unchanged')
            if not isinstance(row, dict) or set(row) != fields:
                raise ValueError('invalid control polygon measurement fields')
            key = (row['family'], row['id'])
            if key not in wanted or key in seen:
                raise ValueError('unexpected or duplicate control polygon comparison')
            seen.add(key)
            if type(row['success']) is not bool or row['success'] != wanted[key]:
                raise ValueError('native command outcome differs')
            if any(row.get(k) is not True for k in ['metadata_exact', 'selection_exact', 'history_exact', 'qualified']):
                raise ValueError('unqualified control polygon state')
            if kind == 'command' and row.get('geometry_unchanged') is not True:
                raise ValueError('source geometry changed')
            error = row['maximum_error']
            if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6:
                raise ValueError('control polygon geometry or channel error')
        if seen != set(wanted):
            raise ValueError('missing control polygon comparisons')
        streams[kind] = sorted(rows, key=lambda r: (r['family'], r['id']))
    paths = list(SOURCES)
    for family in FAMILIES:
        paths += ['tools/rhino_oracle/'+folder+'/'+family+'.json' for folder in ['fixtures', 'observations']]
        paths += [str(p.relative_to(ROOT)) for p in sorted((HERE/'fixtures'/family).rglob('*.3dm'))]
        native = json.loads((HERE/'observations'/(family+'.json')).read_text())
        paths += ['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz' for d in native['provenance']['sources'].values()]
    return dict(native_engine_version='8.32.26160.13001', compared_workflows=len(wanted),
                successful_workflows=sum(wanted.values()), rejected_workflows=sum(not v for v in wanted.values()),
                diagnostic_workflows=diagnostics, epsilon=1e-6,
                maximum_error=max(r['maximum_error'] for rows in streams.values() for r in rows),
                whole_object_command_qualified=True, interactive_object_getter_qualified=True,
                full_native_parity=False, performance_qualified=False,
                limits=[
                    'Finite whole-object corpus. Compared fields include geometry, captured vertex channels, names, colors, layers, group memberships, selection and Undo/Redo.',
                    'Malformed macro trials are diagnostic. TargetObject remains a documented input-layer alias but was rejected by the captured native version.',
                    'Subcurve and face-component picking, texture display and comparative performance remain unqualified.',
                ], comparisons=streams,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('ui_log', type=Path)
    args = parser.parse_args()
    (ROOT/'docs/control-polygon-qualification.json').write_text(json.dumps(
        summarize(args.command_log.read_text(), args.ui_log.read_text()), indent=2, allow_nan=False)+'\n')


if __name__ == '__main__':
    main()
