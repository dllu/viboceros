"""Native tangent-wall topology and independent kernel continuity regressions."""
import argparse
import hashlib
import json
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_surface_apex_native import SOURCES as BASE_SOURCES, load_capture, measured
from .surface_apex_tangent_cases import request, angle_request, threshold_request

ROOT = Path(__file__).resolve().parents[2]
SOURCES = with_shared_sources(BASE_SOURCES+[
    'Cargo.lock',
    'crates/viboceros-oracle/src/object_source.rs',
    'crates/viboceros-oracle/src/plane_arrays.rs',
    'crates/viboceros-smlib/native/surface_apex_extrusion.h',
    'crates/viboceros-smlib/native/CMakeLists.txt', 'crates/viboceros-smlib/build.rs',
    'crates/viboceros-smlib/tests/curve_continuity.rs',
    'crates/viboceros-smlib/tests/surface_apex_extrusion.rs',
    'crates/viboceros-opennurbs/native/curve_discontinuities.h',
    'crates/viboceros-opennurbs/native/curve_discontinuity_query.h',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-opennurbs/src/curve/discontinuities.rs',
    'crates/viboceros-opennurbs/src/curve.rs',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-smlib/Cargo.toml',
    'third_party/usd-brep/source/SMLib/src/SmGeomUtility.cpp',
    'third_party/patches/usd-brep-curve-continuity.patch',
    'third_party/patches/usd-brep-curve-continuity.cmake',
    'tools/usd_brep/curve_continuity_probe.cpp',
    'tools/rhino_oracle/surface_apex_tangent_cases.py',
    'tools/rhino_oracle/generate_surface_apex_tangents.py',
    'tools/rhino_oracle/report_surface_apex_tangents.py',
    'tools/rhino_oracle/test_surface_apex_tangents.py',
    'tools/rhino_oracle/surface_apex_comparison.py',
    'tools/rhino_oracle/client.py',
    'tools/rhino_oracle/audit.py',
])


def native():
    rows, paths, versions = {}, set(), []
    for family, recipe in [('surface_apex_tangents', request),
                           ('surface_apex_tangent_angles', angle_request),
                           ('surface_apex_tangent_threshold', threshold_request)]:
        data, current = load_capture(family, family, recipe)
        for row in data['results']:
            if row['id'] in rows:
                raise ValueError('duplicate surface apex tangent observation')
            rows[row['id']] = row['value']
        paths.update(current)
        versions.append(data['engine_version'])
    if len(rows) != 168:
        raise ValueError('incomplete surface apex tangent corpus')
    return rows, paths, versions


def continuity(data):
    if set(data) != {'baseline', 'patched'}:
        raise ValueError('both kernel continuity implementations are required')
    for label, rows in data.items():
        if len(rows) != 540 or [r['case'] for r in rows] != list(range(540)):
            raise ValueError('kernel continuity coverage differs')
        for row in rows:
            if any(type(row[k]) is not bool for k in ['passed', 'geometric_c2', 'expected']):
                raise ValueError('invalid kernel continuity result')
            if row['passed'] is not (row['geometric_c2'] == row['expected']):
                raise ValueError('kernel continuity claim differs')
            if row['expected'] is not (row['case'] % 5 == (row['case']//5) % 2):
                raise ValueError('analytic curvature vectors differ')
            value = row['classification']
            if type(value) is not int or not 1 <= value <= 13 or row['geometric_c2'] is not (value in [5,6,8,9,10,11,12,13]):
                raise ValueError('kernel continuity classification differs')
        if sum(r['expected'] for r in rows) != 108:
            raise ValueError('analytic continuity cases differ')
        if label == 'patched' and not all(r['passed'] for r in rows):
            raise ValueError('patched kernel continuity failed')
    if [r['expected'] for r in data['baseline']] != [r['expected'] for r in data['patched']]:
        raise ValueError('continuity inputs differ between implementations')
    failed = sum(not r['passed'] for r in data['baseline'])
    if failed != 96:
        raise ValueError('pinned baseline continuity failure count differs')
    return dict(cases=540, baseline_failed=failed, patched_failed=0, observations=data)


def api_measurements(reports, expected):
    operations=[]
    for report in reports:
        if (report.get('protocol_version') != 1 or report.get('absolute_epsilon') != 1e-6
                or report.get('relative_epsilon') != 0. or report.get('passed') is not True
                or report.get('native_failed') != 0 or report.get('mismatched') != 0
                or report.get('matched') != report.get('cases') or len(report.get('operations',[])) != report.get('cases')):
            raise ValueError('surface apex API replay failed')
        operations.extend(report['operations'])
    ids=[row['id'] for row in operations]
    if len(ids)!=len(set(ids)) or set(ids)!=set(expected):
        raise ValueError('surface apex API replay coverage differs')
    for row in operations:
        comparison=row['comparison']
        error=comparison['max_absolute_error']
        if (row['passed'] is not True or row['native_error'] is not None or comparison['id']!=row['id']
                or comparison['passed'] is not True or comparison['differences'] or type(error) not in (int,float)
                or not 0. <= error <= 1e-6):
            raise ValueError('surface apex API comparison differs')
    return reports


def summarize(command_log, app_log, kernel, api):
    expected, paths, versions = native()
    command = measured(command_log, 'SURFACE_APEX_TANGENTS_COMMAND', expected, True)
    app = measured(app_log, 'SURFACE_APEX_TANGENTS_APP', expected, True)
    return dict(scope='SplitAtTangents face counts, explicit apex bounds/volumes, source retention and Undo/Redo; full selection/getter/attribute equivalence and continuous geometry bounds are not established',
        native_versions=versions, native_cases=len(expected), command_replayed=len(command), app_replayed=len(app),
        absolute_epsilon=1e-6, maximum_error=max(r[p]['maximum_error'] for rows in [command,app] for r in rows for p in ['after','undo','redo']),
        full_rhino_parity=False, performance_qualified=False, mismatches=0,
        api_replayed=len(expected), api=api_measurements(api,expected),
        kernel_continuity=continuity(kernel), command=command, app=app,
        sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('app_log', type=Path)
    parser.add_argument('kernel', type=Path)
    parser.add_argument('api', type=Path, nargs=3)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/surface-apex-tangents-qualification.json')
    args = parser.parse_args()
    result = summarize(args.command_log.read_text(), args.app_log.read_text(), json.loads(args.kernel.read_text()),
                       [json.loads(p.read_text()) for p in args.api])
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    print('Surface apex tangents: {} commands, {} GUI; 540 analytic kernel cases passed'.format(result['command_replayed'], result['app_replayed']))


if __name__ == '__main__':
    main()
