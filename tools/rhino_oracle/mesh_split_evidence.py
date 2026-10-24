"""Immutable split-family requests, archived producers, and original 3DM scenes."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .mesh_split_cases import FAMILIES, request
from .report_mesh_split_kernel import validate_ngons

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def native(families, sources, *, case_families=FAMILIES, request_factory=request,
           prefix='mesh_split_', allow_unknown=()):
    rows, versions, paths = {}, set(), set(sources)
    for family in families:
        cases = case_families[family]
        name = prefix + family
        query, capture = [HERE / d / (name + '.json') for d in ['fixtures', 'observations']]
        paths.update(str(p.relative_to(ROOT)) for p in [query, capture])
        data = json.loads(capture.read_text())
        if (json.loads(query.read_text()) != request_factory(family) or
                data['provenance']['request_sha256'] != hashlib.sha256(query.read_bytes()).hexdigest()):
            raise ValueError('mesh split fixed request differs')
        for sha in data['provenance']['sources'].values():
            archive = HERE / 'observations/source_snapshots' / (sha + '.gz')
            if hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest() != sha:
                raise ValueError('mesh split archived producer differs')
            paths.add(str(archive.relative_to(ROOT)))
        if [r['id'] for r in data['results']] != list(cases):
            raise ValueError('mesh split capture coverage differs')
        versions.add(data['engine_version'])
        for row in data['results']:
            identifier, value = row['id'], row['value']
            if (value['input'] != cases[identifier] or
                    'Unknown command:' in value['history'] and identifier not in allow_unknown):
                raise ValueError('mesh split native input differs')
            scene = HERE / 'fixtures' / name / (identifier + '.3dm')
            if scene.read_bytes() != base64.b64decode(value['source_3dm']):
                raise ValueError('mesh split source archive differs')
            paths.add(str(scene.relative_to(ROOT)))
            for stage in ['before', 'after', 'undo', 'redo']:
                for obj in value[stage]:
                    if 'ngons' in obj.get('mesh', {}):
                        validate_ngons(obj['mesh'])
            rows[identifier] = value
    if len(versions) != 1 or len(rows) != sum(len(case_families[f]) for f in families):
        raise ValueError('mesh split capture domain differs')
    return versions.pop(), rows, paths
