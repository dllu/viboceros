"""Rebuild native curve-path geometry checks and explicit failure-policy differences."""
import argparse
import hashlib
import json
from pathlib import Path
from .curve_path_comparison import canonical
from .curve_path_extrusion_cases import request, continuity_request
from .qualification_sources import with_shared_sources
from .report_surface_apex_native import compare, load_capture

ROOT=Path(__file__).resolve().parents[2]
SOURCES=with_shared_sources([
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/crease_splitting.rs',
    'crates/viboceros-command/src/crease_splitting/tests.rs',
    'crates/viboceros-command/src/extrude_along_curve.rs',
    'crates/viboceros-command/src/extrude_along_curve/native.rs',
    'crates/viboceros-command/src/extrude_along_curve/options.rs',
    'crates/viboceros-command/src/extrude_along_curve/topology.rs',
    'crates/viboceros-command/src/tests/surfaces/extrusions.rs',
    'crates/viboceros-command/src/tests/document/builtin_commands.txt',
    'crates/viboceros-geometry/src/nurbs.rs',
    'crates/viboceros-geometry/src/lib.rs',
    'crates/viboceros-geometry/src/nurbs/discontinuities.rs',
    'crates/viboceros-geometry/src/error.rs',
    'crates/viboceros-geometry/src/brep/surface_grid.rs',
    'crates/viboceros-geometry/src/brep/cap.rs',
    'crates/viboceros-geometry/src/brep/cap/boundary.rs',
    'crates/viboceros-opennurbs/src/curve/discontinuities.rs',
    'crates/viboceros-opennurbs/src/curve.rs',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/native/curve_discontinuities.h',
    'crates/viboceros-opennurbs/native/curve_discontinuity_query.h',
    'crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-oracle/src/lib.rs',
    'crates/viboceros-oracle/src/curve_native.rs',
    'crates/viboceros-oracle/src/curve_path_extrusion.rs',
    'crates/viboceros-oracle/tests/curve_path_native.rs',
    'crates/viboceros-oracle/tests/support/curve_path.rs',
    'crates/viboceros-oracle/tests/support/compare.rs',
    'src/app.rs', 'src/app/tests/mod.rs', 'src/app/tests/extrude_curve_path_native.rs',
    'tools/rhino_oracle/curve_path_extrusion_probe.py',
    'tools/rhino_oracle/curve_path_extrusion_cases.py',
    'tools/rhino_oracle/curve_path_comparison.py',
    'tools/rhino_oracle/generate_curve_path_extrusion.py',
    'tools/rhino_oracle/report_curve_path_extrusion.py',
    'tools/rhino_oracle/test_curve_path_extrusion.py',
    'tools/rhino_oracle/client.py', 'tools/rhino_oracle/structural_sdk.py',
    'tools/rhino_oracle/rhino_worker.py', 'docs/smlib-translational-sweep.md',
])


def native():
    rows={};operations={};paths=set();versions=[]
    for family,recipe in [('curve_path_extrusion',request),('curve_path_continuity',continuity_request)]:
        data,files=load_capture(family,family,recipe)
        paths.update(files);versions.append(data['engine_version'])
        for op,row in zip(recipe()['operations'],data['results']):
            if row['id'] in rows:raise ValueError('duplicate curve path observation')
            rows[row['id']]=canonical(op,row['value'],True)
            operations[row['id']]=op
    if len(rows)!=272 or sum(r['succeeded'] for r in rows.values())!=264:
        raise ValueError('incomplete curve path corpus')
    return rows,operations,paths,versions


def measurements(log,prefix,expected):
    read=lambda label:[json.loads(line.split(label+' ',1)[1]) for line in log.splitlines() if label+' ' in line]
    passed=read(prefix);failed=read('CURVE_PATH_FAILURE' if prefix=='CURVE_PATH_COMMAND' else prefix+'_FAILURE')
    ids=[r['id']for r in passed+failed]
    if len(ids)!=len(set(ids)) or set(ids)!=set(expected):raise ValueError('curve path replay coverage differs')
    for row in passed:
        target=expected[row['id']]
        if target['succeeded'] is not True or row['maximum_error']!=compare(row['value'],target):
            raise ValueError('curve path successful replay differs')
    failure_rows=[]
    for row in failed:
        target=expected[row['id']]
        actual=row['actual']
        if (target['succeeded'] is not False or actual!={'succeeded':False,'after':[{'source':0,'unchanged':True},{'source':1,'unchanged':True}],'undo':None,'redo':None}):
            raise ValueError('curve path failure is not atomic')
        retained=any(r['source']==0 for r in target['after'])
        failure_rows.append(dict(id=row['id'],native_profile_retained=retained,viboceros_profile_retained=True,
                                 matches_native_source_retention=retained,actual=actual,native=target))
    if len(passed)!=264 or len(failed)!=8 or sum(not r['matches_native_source_retention']for r in failure_rows)!=4:
        raise ValueError('curve path failure policy coverage differs')
    return dict(successful=sorted(passed,key=lambda r:r['id']),failed=sorted(failure_rows,key=lambda r:r['id']))


def api_measurements(reports,expected):
    if reports is None:return None
    if not isinstance(reports,list) or len(reports)!=2:raise ValueError('both curve path API families are required')
    ids=[];failures=[]
    for report in reports:
        if (report.get('protocol_version')!=1 or report.get('absolute_epsilon')!=1e-6
                or report.get('relative_epsilon')!=0. or not isinstance(report.get('operations'),list)):
            raise ValueError('invalid curve path API comparison')
        for row in report['operations']:
            ids.append(row['id'])
            if type(row.get('passed')) is not bool:raise ValueError('invalid curve path API result')
            if row['passed']:
                if row.get('differences') or not 0.<=row.get('max_absolute_error',-1.)<=1e-6:
                    raise ValueError('curve path API metric differs')
            else:
                failures.append(row['id'])
                if not row.get('differences'):raise ValueError('curve path API failure has no diagnostic')
        if report.get('passed') is not all(r['passed']for r in report['operations']):
            raise ValueError('curve path API summary differs')
    expected_failures={key for key,v in expected.items() if not v['succeeded'] and not any(r['source']==0 for r in v['after'])}
    if len(ids)!=len(set(ids)) or set(ids)!=set(expected) or set(failures)!=expected_failures:
        raise ValueError('curve path API coverage or failure policy differs')
    return reports


def summarize(command_log,app_log,api=None,portable_log=None):
    expected,operations,paths,versions=native()
    command=measurements(command_log,'CURVE_PATH_COMMAND',expected)
    app=measurements(app_log,'CURVE_PATH_APP',expected)
    return dict(scope='Actual command model geometry, retained source identity, and successful Undo/Redo. Native helper selection, UI picking, attributes, continuous surface distance and performance parity are not qualified.',
        native_cases=len(expected),successful_native_commands=264,failed_native_commands=8,
        native_versions=versions,absolute_epsilon=1e-6,relative_epsilon=0.,
        maximum_error=max(r['maximum_error']for data in [command,app]for r in data['successful']),
        successful_geometry_mismatches=0,failure_source_retention_differences=4,
        full_rhino_parity=False,performance_qualified=False,
        failure_policy='Viboceros rolls back all source edits on failure; Rhino deleted the profile in four captured failures.',
        command=command,app=app,
        python_api=api_measurements(api,expected),
        portable=measurements(portable_log,'CURVE_PATH_COMMAND',expected) if portable_log is not None else None,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sorted(paths|set(SOURCES))})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('app_log',type=Path)
    parser.add_argument('--api',type=Path);parser.add_argument('--portable-log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/curve-path-extrusion-qualification.json')
    args=parser.parse_args();result=summarize(args.command_log.read_text(),args.app_log.read_text(),
        json.loads(args.api.read_text()) if args.api else None,args.portable_log.read_text() if args.portable_log else None)
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Curve path: 264 successful commands and 8 atomic failures per API/GUI; 4 native failure-retention differences')


if __name__=='__main__':main()
