"""Recompute MeshIntersect command/getter comparisons from frozen native scenes."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from . import report_mesh_boolean_commands as baseline
from .mesh_intersect_cases import CASES, CONTROLS, MESH_MODES, COVERAGE, TOPOLOGY, request
from .qualification_sources import with_shared_sources

ROOT,HERE=baseline.ROOT,baseline.HERE
GAPS=['mesh_intersect_identical_post','mesh_intersect_identical_pre',
      'mesh_intersect_modes_mesh_identical']
SOURCES=with_shared_sources(baseline.SOURCES+[
    'crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-smlib/native/CMakeLists.txt',
    'crates/viboceros-smlib/build.rs',
    'docs/commands/mesh-intersect.md',
    'crates/viboceros-command/tests/mesh_intersect_native.rs',
    'crates/viboceros-command/tests/support/mesh_intersect.rs',
    'crates/viboceros-smlib/tests/mesh_intersections.rs',
    'tools/rhino_oracle/report_mesh_intersect.py',
    'tools/rhino_oracle/test_mesh_intersect.py',
    'tools/rhino_oracle/mesh_intersect_probe.py',
    'tools/rhino_oracle/mesh_intersect_cases.py',
    'tools/rhino_oracle/generate_mesh_intersect.py',
])


def native():
    expected,exploratory,versions,paths={},{},set(),set(SOURCES)
    for family,cases,payload in [('discovery',CASES,request()),('controls',CONTROLS,request(True)),('modes',MESH_MODES,request(mesh_modes=True)),('coverage',COVERAGE,request(coverage=True)),('topology',TOPOLOGY,request(topology=True))]:
        name='mesh_intersect_'+family
        query,capture=[HERE/folder/(name+'.json') for folder in ['fixtures','observations']]
        data=json.loads(capture.read_text())
        if json.loads(query.read_text())!=payload or hashlib.sha256(query.read_bytes()).hexdigest()!=data['provenance']['request_sha256']:
            raise ValueError('MeshIntersect fixed request changed')
        paths.update(str(p.relative_to(ROOT))for p in [query,capture])
        for sha in data['provenance']['sources'].values():
            path=HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest()!=sha:
                raise ValueError('MeshIntersect archived producer changed')
            paths.add(str(path.relative_to(ROOT)))
        if [r['id']for r in data['results']]!=list(cases):raise ValueError('MeshIntersect native coverage differs')
        versions.add(data['engine_version'])
        for row in data['results']:
            identifier,value=row['id'],row['value']
            if value['input']!=cases[identifier]:raise ValueError('MeshIntersect native input changed')
            path=HERE/'fixtures'/name/(identifier+'.3dm')
            if path.read_bytes()!=base64.b64decode(value['source_3dm']):raise ValueError('MeshIntersect source file changed')
            paths.add(str(path.relative_to(ROOT)))
            if 'Unknown command:' in value['history']:exploratory[identifier]='native rejected option spelling'
            else:expected[identifier]=value
    if len(versions)!=1 or (len(expected),len(exploratory))!=(60,3):raise ValueError('MeshIntersect capture domain differs')
    return versions.pop(),expected,exploratory,paths


def summarize(command_log,getter_log):
    version,expected,exploratory,paths=native()
    commands=baseline.measurements(command_log,'MESH_INTERSECT_COMMAND',{
        k:v for k,v in expected.items()if not v['input'].get('cancel')and not k.endswith('_menu')})
    getters=baseline.measurements(getter_log,'MESH_INTERSECT_GETTER',expected)
    gaps=[baseline.gap_names(rows,expected)for rows in [commands,getters]]
    if gaps!=[GAPS,GAPS]:raise ValueError('MeshIntersect compatibility gaps changed')
    return dict(scope='MeshIntersect curves, shared tolerance/coplanar preferences and document history',
        native_version=version,epsilon=baseline.EPSILON,
        comparison='per-object order, source identity, attributes, selection, polyline length and bidirectional endpoint/midpoint samples after execution, Undo and Redo; meshes additionally compare area and oriented volume',
        command_replayed=len(commands),getter_replayed=len(getters),
        command_matched=len(commands)-len(GAPS),getter_matched=len(getters)-len(GAPS),
        command_compatibility_gaps=GAPS,getter_compatibility_gaps=GAPS,
        exploratory_workflows=exploratory,
        pending=['coincident triangle-edge traversal, native polyline vertex segmentation and parameter domains',
                 'curved display-mesh agreement, extreme scales, custom intersection coefficients and comparative performance'],
        commands=commands,getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('getter_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/mesh-intersect-qualification.json')
    args=parser.parse_args();record=summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('MeshIntersect matches: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__=='__main__':main()
