"""Compare measured outline objects, metadata and history; expose surface mesh gaps."""
import argparse
import base64
import gzip
import hashlib
import json
import math
from pathlib import Path
from .mesh_outline_command_cases import COMMANDS, LIFECYCLE, QUADS, request
from .report_mesh_outline import HERE, ROOT, EPSILON, boundary_error, contour
from .qualification_sources import with_shared_sources

SURFACE_GAPS = {'outline_lifecycle_sphere'}
PROJECTION_GAPS = {'outline_lifecycle_warped_perspective', 'outline_quads_post'}
GAPS = SURFACE_GAPS | PROJECTION_GAPS
FIELDS = ['source','role','selected','name','layer','color','color_source','groups']
SOURCES = with_shared_sources([
    'Cargo.lock','crates/viboceros-command/src/lib.rs','crates/viboceros-command/src/mesh_outline.rs',
    'crates/viboceros-command/src/interface.rs','crates/viboceros-command/src/picking.rs',
    'crates/viboceros-command/tests/mesh_outline_command_native.rs',
    'crates/viboceros-command/tests/support/mesh_outline.rs',
    'crates/viboceros-geometry/src/mesh.rs','crates/viboceros-geometry/tests/support/mesh_outline.rs',
    'crates/viboceros-document/src/lib.rs','crates/viboceros-document/src/history.rs',
    'crates/viboceros-document/src/selection.rs','src/app.rs','src/app/object_selection.rs',
    'src/app/object_selection/mesh_outline_tests.rs','src/viewport.rs','src/viewport/named_view.rs',
    'docs/commands/mesh-outline.md',
] + ['tools/rhino_oracle/'+name for name in [
    'mesh_outline_command_cases.py','mesh_outline_command_probe.py','mesh_outline_cases.py',
    'generate_mesh_outline.py','report_mesh_outline.py','report_mesh_outline_command.py',
    'test_mesh_outline_command.py','client.py','rhino_worker.py','structural_sdk.py',
    'join_probe.py','merge_edges_probe.py','qualification_sources.py']])


def native():
    references, versions, paths = {}, set(), set(SOURCES)
    for family, flags, cases in [('mesh_outline_commands',{},COMMANDS),
                                 ('mesh_outline_lifecycle',{'lifecycle':True},LIFECYCLE),
                                 ('mesh_outline_quads',{'quads':True},QUADS)]:
        query=HERE/'fixtures'/(family+'.json');capture=HERE/'observations'/(family+'.json')
        paths.update(str(p.relative_to(ROOT)) for p in [query,capture])
        if json.loads(query.read_text()) != request(**flags):
            raise ValueError('fixed outline command request changed')
        data=json.loads(capture.read_text());versions.add(data['engine_version'])
        if data['provenance']['request_sha256'] != hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('outline command request hash differs')
        for digest in data['provenance']['sources'].values():
            archive=HERE/'observations/source_snapshots'/(digest+'.gz')
            if hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest()!=digest:
                raise ValueError('outline command frozen producer differs')
            paths.add(str(archive.relative_to(ROOT)))
        if [r['id'] for r in data['results']]!=list(cases):
            raise ValueError('outline command native coverage differs')
        for row in data['results']:
            v=row['value'];name=row['id']
            if v['input']!=cases[name]:raise ValueError('outline command input differs')
            path=HERE/'fixtures'/family/(name+'.3dm')
            if path.read_bytes()!=base64.b64decode(v['source_3dm'],validate=True):
                raise ValueError('saved outline source differs')
            paths.add(str(path.relative_to(ROOT)))
            events=[e for e in v['events']if e['name']=='MeshOutline']
            if len(events)!=1 or events[0]['result']!=('Success'if v['success']else'Cancel'):
                raise ValueError('native outline event differs')
            if flags:
                if set(v['views'])!={'before','after','after_script','undo','redo'}:
                    raise ValueError('getter-time outline camera capture missing')
                references[name]=v
    if len(versions)!=1:raise ValueError('mixed native outline versions')
    return versions.pop(),references,paths


def sampled_boundary_distance(actual, expected):
    def samples(paths):
        for path in paths:
            yield from path
            for a,b in zip(path,path[1:]):yield [(x+y)*.5 for x,y in zip(a,b)]
    def point_segment(p,a,b):
        d=[y-x for x,y in zip(a,b)];den=sum(x*x for x in d)
        t=max(0.,min(1.,sum((y-x)*z for x,y,z in zip(a,p,d))/den))if den else 0.
        return math.dist(p,[x+t*y for x,y in zip(a,d)])
    def directed(a,b):
        edges=[(p,q)for path in b for p,q in zip(path,path[1:])]
        return max((min(point_segment(p,a,b)for a,b in edges)for p in samples(a)),default=0.)
    if not actual or not expected:return 0. if actual==expected else None
    return max(directed(actual,expected),directed(expected,actual))


def stage(actual, expected):
    if not isinstance(actual,list)or len(actual)!=len(expected):
        raise ValueError('outline document object count differs')
    for a,n in zip(actual,expected):
        if any(a[k]!=n[k]for k in FIELDS):
            raise ValueError('outline identity, order, metadata or selection differs')
    a=[r['polyline']for r in actual if r['role']=='output']
    n=[r['polyline']for r in expected if r['role']=='output']
    for path in a+n:contour(path)
    try:error=boundary_error(a,n)
    except ValueError:error=None
    return error,sampled_boundary_distance(a,n)


def summarize(text):
    version,refs,paths=native();stages={};maximum=0.
    for label in ['COMMAND','GETTER']:
        required={k for k,v in refs.items()if label=='GETTER'or v['success']}
        rows={};gaps=set();control_gaps=set()
        for line in text.splitlines():
            prefix='MESH_OUTLINE_'+label+' '
            if prefix not in line:continue
            row=json.loads(line.split(prefix,1)[1]);name=row['id']
            if name not in required or name in rows:raise ValueError('measured outline coverage differs')
            if row['source_unchanged']is not True:raise ValueError('outline source changed')
            errors=[];sampled=[]
            for key in ['after','undo','redo']:
                error,distance=stage(row[key],refs[name][key]);errors.append(error);sampled.append(distance)
            matched=all(e is not None and e<=EPSILON for e in errors)
            error=max(errors)if all(e is not None for e in errors)else None
            if row['geometry']is not matched or row['maximum_error']!=error:
                # Rust and Python hypot implementations may round differently.
                if (row['geometry']is not matched or error is None or row['maximum_error']is None
                        or abs(error-row['maximum_error'])>1e-14):
                    raise ValueError('outline declared result differs from measured geometry')
            if label=='COMMAND':
                for path in row['native_render_control']:contour(path)
                try:
                    control=boundary_error(row['native_render_control'],
                        [o['polyline']for o in refs[name]['after']if o['role']=='output'])
                except ValueError:control=None
                if control is None:
                    if row['native_render_control_error']is not None:
                        raise ValueError('native render mesh projection control differs')
                    control_gaps.add(name)
                elif (row['native_render_control_error']is None or not math.isfinite(row['native_render_control_error'])
                        or abs(control-row['native_render_control_error'])>1e-14):
                    raise ValueError('native render mesh projection control differs')
                elif control>EPSILON:control_gaps.add(name)
            if not matched:gaps.add(name)
            if matched:maximum=max(maximum,error)
            rows[name]=dict(row,maximum_error=error,maximum_sampled_boundary_deviation=max(d for d in sampled if d is not None))
        if set(rows)!=required or gaps!=GAPS:
            raise ValueError('outline measured coverage or render tessellation gap set differs')
        if label=='COMMAND'and control_gaps!=PROJECTION_GAPS:
            raise ValueError('outline projection control gap set differs')
        stages[label.lower()]=[rows[name]for name in sorted(rows)]
    return dict(scope='MeshOutline command and getter lifecycle; render tessellation and small perspective quad fidelity incomplete',
                native_version=version,epsilon=EPSILON,native_command_workflows=71,
                exploratory_workflows_with_only_initial_viewport=32,
                replayed_commands=len(stages['command']),replayed_getters=len(stages['getter']),
                fully_matched_commands=len(stages['command'])-len(GAPS),fully_matched_getters=len(stages['getter'])-len(GAPS),
                surface_render_mesh_gaps=sorted(SURFACE_GAPS),projection_gaps=sorted(PROJECTION_GAPS),maximum_qualified_error=maximum,
                pending=['curved surface render tessellation parity','small perspective warped quad projection parity','native eye-plane and extreme range qualification'],
                **stages,sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('logs',nargs='+',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/mesh-outline-command-qualification.json')
    args=parser.parse_args();result=summarize('\n'.join(p.read_text()for p in args.logs))
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Compared {} commands and {} getters; one render tessellation and two perspective projection gaps remain'.format(result['replayed_commands'],result['replayed_getters']))


if __name__=='__main__':main()
