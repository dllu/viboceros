"""Recompute self-intersection geometry, selection, options, and history evidence."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from . import report_mesh_boolean_commands as baseline
from .mesh_self_intersect_cases import FAMILIES, request
from .qualification_sources import with_shared_sources

ROOT,HERE=baseline.ROOT,baseline.HERE
GAPS=['mesh_self_intersect_controls_boolean_document', 'mesh_self_intersect_controls_identical_boxes']
SOURCES=with_shared_sources(baseline.SOURCES+[
    'crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-smlib/native/CMakeLists.txt','crates/viboceros-smlib/build.rs',
    'crates/viboceros-smlib/tests/mesh_self_intersections.rs',
    'crates/viboceros-command/tests/mesh_self_intersect_native.rs',
    'crates/viboceros-command/tests/support/mesh_self_intersect.rs',
    'docs/commands/mesh-self-intersect.md',
]+['tools/rhino_oracle/'+name for name in [
    'mesh_self_intersect_cases.py','mesh_self_intersect_probe.py',
    'generate_mesh_self_intersect.py','report_mesh_self_intersect.py','test_mesh_self_intersect.py']])


def native():
    rows,versions,paths={},set(),set(SOURCES)
    for family,cases in FAMILIES.items():
        name='mesh_self_intersect_'+family
        query,capture=[HERE/d/(name+'.json')for d in ['fixtures','observations']]
        paths.update(str(p.relative_to(ROOT))for p in [query,capture])
        data=json.loads(capture.read_text())
        if json.loads(query.read_text())!=request(family) or data['provenance']['request_sha256']!=hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('self-intersection request changed')
        for sha in data['provenance']['sources'].values():
            archive=HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest()!=sha:
                raise ValueError('self-intersection producer changed')
            paths.add(str(archive.relative_to(ROOT)))
        if [r['id']for r in data['results']]!=list(cases):
            raise ValueError('self-intersection native coverage differs')
        versions.add(data['engine_version'])
        for row in data['results']:
            identifier,value=row['id'],row['value']
            if value['input']!=cases[identifier] or 'Unknown command:'in value['history']:
                raise ValueError('self-intersection native input differs')
            scene=HERE/'fixtures'/name/(identifier+'.3dm')
            if scene.read_bytes()!=base64.b64decode(value['source_3dm']):
                raise ValueError('self-intersection source file changed')
            paths.add(str(scene.relative_to(ROOT)))
            rows[identifier]=value
    if len(versions)!=1 or len(rows)!=39:raise ValueError('self-intersection capture domain differs')
    return versions.pop(),rows,paths


def summarize(command_log,getter_log):
    version,expected,paths=native()
    commands=baseline.measurements(command_log,'MESH_SELF_INTERSECT_COMMAND',{
        k:v for k,v in expected.items()if not v['input'].get('cancel')and not k.endswith('_menu')})
    getters=baseline.measurements(getter_log,'MESH_SELF_INTERSECT_GETTER',expected)
    if any(baseline.gap_names(rows,expected)!=GAPS for rows in [commands,getters]):
        raise ValueError('self-intersection compatibility gaps changed')
    return dict(scope='MeshSelfIntersect per-object curves, shared options, source ownership and document history',
        native_version=version,epsilon=baseline.EPSILON,
        comparison='ordered objects, source identity, attributes, selection, curve length and bidirectional endpoint/midpoint samples after execution, Undo and Redo',
        command_replayed=len(commands),getter_replayed=len(getters),
        command_matched=len(commands)-len(GAPS),getter_matched=len(getters)-len(GAPS),
        command_compatibility_gaps=GAPS,getter_compatibility_gaps=GAPS,
        pending=['coincident closed-shell preprocessing and coincident mesh Boolean control',
                 'native polyline segmentation and parameter domains',
                 'curved display-mesh agreement, near contacts, extreme scales and comparative performance'],
        commands=commands,getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sorted(paths)})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('getter_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/mesh-self-intersect-qualification.json')
    args=parser.parse_args()
    record=summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('MeshSelfIntersect: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__=='__main__':main()
