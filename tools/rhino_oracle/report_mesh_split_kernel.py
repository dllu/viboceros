"""Recompute open-mesh partition comparisons against frozen Rhino evidence."""
import argparse
import base64
import gzip
import hashlib
import json
import math
from pathlib import Path
from .mesh_split_cases import FAMILIES,request
from .mesh_boolean_comparison import cross,sub,measures,sampled_boundary_distance,triangles
from .qualification_sources import with_shared_sources

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
EPSILON=1e-7
GAPS=['mesh_split_controls_mixed_cutter','mesh_split_controls_three_cutters']
SOURCES=with_shared_sources([
    'Cargo.toml','Cargo.lock','crates/viboceros-smlib/Cargo.toml',
    'crates/viboceros-smlib/src/lib.rs','crates/viboceros-smlib/src/native.rs',
    'crates/viboceros-smlib/native/bridge.h','crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-smlib/native/CMakeLists.txt','crates/viboceros-smlib/build.rs',
    'crates/viboceros-smlib/tests/mesh_split.rs','crates/viboceros-smlib/tests/support/mesh_boolean.rs',
    'crates/viboceros-geometry/tests/support/mesh_outline.rs','docs/mesh-split-kernel.md',
]+['tools/rhino_oracle/'+name for name in [
    'mesh_split_cases.py','mesh_split_probe.py','mesh_split_sdk.py','generate_mesh_split.py',
    'mesh_self_intersect_cases.py','mesh_boolean_command_probe.py','mesh_boolean_command_cases.py',
    'mesh_boolean_option_cases.py','mesh_boolean_sets_cases.py','mesh_empty_capture.py',
    'mesh_boolean_cases.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py',
    'structural_sdk.py','qualification_sources.py','mesh_boolean_comparison.py',
    'report_mesh_split_kernel.py','test_mesh_split_kernel.py']])


def oriented_area(meshes):
    result=[0.,0.,0.]
    for a,b,c in triangles(meshes):
        normal=cross(sub(b,a),sub(c,a))
        result=[value+component*.5 for value,component in zip(result,normal)]
    return result


def same_part(a,b):
    area,volume=measures([a]);other_area,other_volume=measures([b])
    distance=sampled_boundary_distance([a],[b])
    return all(abs(a-b)<=EPSILON for a,b in zip(oriented_area([a]),oriented_area([b]))) and abs(area-other_area)<=EPSILON and abs(volume-other_volume)<=EPSILON and distance is not None and distance<=EPSILON


def same_parts(actual,expected):
    if len(actual)!=len(expected):return False
    used=set()
    for a in actual:
        index=next((i for i,b in enumerate(expected)if i not in used and same_part(a,b)),None)
        if index is None:return False
        used.add(index)
    return True


def validate_ngons(mesh):
    list(triangles([mesh]))
    for ngon in mesh['ngons']:
        for field,limit in [('vertices',len(mesh['vertices'])),('faces',len(mesh['faces']))]:
            indices=ngon[field]
            if not indices or len(set(indices))!=len(indices) or any(type(i)is not int or not 0<=i<limit for i in indices):
                raise ValueError('invalid native split n-gon indices')
        if len(ngon['vertices'])<3:raise ValueError('invalid native split n-gon boundary')


def native():
    expected,versions,paths={},set(),set(SOURCES)
    captured=0
    for family in ['discovery','controls']:
        cases=FAMILIES[family]
        name='mesh_split_'+family
        query,capture=[HERE/f/(name+'.json')for f in ['fixtures','observations']]
        paths.update(str(p.relative_to(ROOT))for p in [query,capture])
        data=json.loads(capture.read_text())
        if json.loads(query.read_text())!=request(family) or data['provenance']['request_sha256']!=hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('fixed mesh split request changed')
        for sha in data['provenance']['sources'].values():
            archive=HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest()!=sha:
                raise ValueError('mesh split archived producer changed')
            paths.add(str(archive.relative_to(ROOT)))
        if [r['id']for r in data['results']]!=list(cases):raise ValueError('mesh split capture domain changed')
        versions.add(data['engine_version'])
        for row in data['results']:
            identifier,value=row['id'],row['value'];case=cases[identifier];captured+=1
            if value['input']!=case:raise ValueError('mesh split native input differs')
            scene=HERE/'fixtures'/name/(identifier+'.3dm')
            if scene.read_bytes()!=base64.b64decode(value['source_3dm']):raise ValueError('mesh split source archive differs')
            paths.add(str(scene.relative_to(ROOT)))
            if family=='controls':
                if len(value['split_sdk'])!=len(case['first']):raise ValueError('mesh split SDK target count differs')
                parts=[]
                for sdk in value['split_sdk']:
                    if sdk['inputs_unchanged']is not True:raise ValueError('mesh split SDK changed a private input')
                    for part in sdk['parts']:validate_ngons(part)
                    if not case['discard_unsplit']or len(sdk['parts'])>1:parts.extend(sdk['parts'])
                command=[o['mesh']for o in value['after']if o['role']=='output']
                if not same_parts(command,parts):raise ValueError('public split SDK and command geometry differ')
                for mesh in command:validate_ngons(mesh)
                expected[identifier]=dict(family=family,parts=[p for s in value['split_sdk']for p in s['parts']])
            elif not(case.get('cancel')or case.get('cancel_second'))and all(s['kind']=='mesh'for s in case['sources']):
                expected[identifier]=dict(family=family,parts=[o['mesh']for o in value['after']if o['role']=='output'])
    if captured!=36 or len(expected)!=30 or len(versions)!=1:raise ValueError('mesh split comparison coverage differs')
    return versions.pop(),expected,paths


def summarize(log):
    version,expected,paths=native();rows={}
    for line in log.splitlines():
        if 'MESH_SPLIT_KERNEL 'not in line:continue
        row=json.loads(line.split('MESH_SPLIT_KERNEL ',1)[1]);name=row['id']
        if name not in expected or name in rows:raise ValueError('unsupported or duplicate mesh split measurement')
        if row['sources_unchanged']is not True or row['error']is not None or row['family']!=expected[name]['family']:
            raise ValueError('mesh split ownership, status or domain differs')
        actual,reference=row['parts'],expected[name]['parts']
        if row['matched']is not same_parts(actual,reference):raise ValueError('mesh split geometry declaration differs')
        area,volume=measures(actual)
        if len(row['oriented_area'])!=3 or any(type(x)not in (float,int)or not math.isfinite(x)or abs(x-y)>1e-8 for x,y in zip(row['oriented_area'],oriented_area(actual))):
            raise ValueError('mesh split oriented area declaration differs')
        for field,value in [('area',area),('signed_volume',volume)]:
            if type(row[field])not in (int,float)or not math.isfinite(row[field])or abs(row[field]-value)>1e-8:
                raise ValueError('mesh split measure declaration differs')
        distance=sampled_boundary_distance(actual,reference);declared=row['maximum_sampled_boundary_deviation']
        if (distance is None)!=(declared is None)or distance is not None and (type(declared)not in (int,float)or not math.isfinite(declared)or abs(distance-declared)>1e-8):
            raise ValueError('mesh split sampled distance declaration differs')
        rows[name]=row
    if set(rows)!=set(expected):raise ValueError('mesh split measurement coverage differs')
    gaps=sorted(name for name,row in rows.items()if not row['matched'])
    if gaps!=GAPS:raise ValueError('mesh split compatibility gaps changed: '+repr(gaps))
    return dict(scope='Open mesh partition kernel; unordered component geometry, no command or UI implementation claim',
        native_version=version,epsilon=EPSILON,captured_workflows=36,replayed=len(rows),matched=len(rows)-len(gaps),
        comparison='Component count, per-component area, oriented area vector, common-origin signed volume and bidirectional vertex/edge-midpoint/triangle-centroid samples; input storage remains unchanged',
        compatibility_gaps=gaps,pending=['vertex channel transfer; command policies and n-gons have a separate qualification',
            'mixed coplanar grouping and native surface truncation with three cutters',
            'native component order, extreme scales, near contacts and comparative performance'],
        measurements=[rows[k]for k in sorted(rows)],
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/mesh-split-kernel-qualification.json')
    args=parser.parse_args();data=summarize(args.log.read_text())
    args.output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    print('Mesh split kernel: {matched}/{replayed} native comparisons'.format(**data))


if __name__=='__main__':main()
