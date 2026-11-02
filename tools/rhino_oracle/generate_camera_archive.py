"""Capture native cached camera frames and retain the exact generated 3DM inputs."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .camera_archive_probe import PROJECTIONS, validate
from .client import OracleClient

HERE = Path(__file__).resolve().parent
FAMILY = 'camera_archive'


def request():
    operations = []
    for projection in PROJECTIONS:
        for label, direction, up in [('skew', [1, 2, -2], [0, 4, 1]),
                                     ('orthogonal', [0, 0, -3], [0, 5, 0]),
                                     ('tilted', [4, 1, -2], [2, -1, 3])]:
            for rotate in (False, True):
                op = dict(op='camera_archive', id=projection+'-'+label+'-'+str(int(rotate)),
                          projection=projection, direction=direction, up=up, rotate=rotate)
                validate(op)
                operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-8, relative=1e-12, angular=1e-8), operations=operations)


def capture(family, query, extra_sources=()):
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if fixture.exists() or output.exists():
        raise ValueError('camera archives are immutable')
    fixture.write_text(json.dumps(query, indent=2) + '\n')
    names = ['generate_camera_archive.py', 'camera_archive_probe.py', 'structural_sdk.py',
             'client.py', 'rhino_worker.py', 'merge_edges_probe.py', *extra_sources]
    frozen = {name: (HERE/name).read_bytes() for name in names}
    print('Capturing {} camera archives'.format(len(query['operations'])), flush=True)
    result = OracleClient(settings_scheme='VibocerosOracleCameraArchiveQualified20261102').run_rhino(query, timeout=900)
    if frozen != {name: (HERE/name).read_bytes() for name in names}:
        raise ValueError('camera archive producers changed during capture')
    if [row['id'] for row in result['results']] != [op['id'] for op in query['operations']]:
        raise ValueError('incomplete archive capture')
    folder = HERE / ('fixtures/' + family)
    folder.mkdir()
    digest = lambda value: hashlib.sha256(value).hexdigest()
    for row in result['results']:
        value = row['value']
        data = base64.b64decode(value.pop('archive_base64'), validate=True)
        path = folder / (row['id'] + '.3dm')
        path.write_bytes(data)
        value['file'] = str(path.relative_to(HERE))
        value['file_sha256'] = digest(data)
    sources = {name: digest(data) for name, data in frozen.items()}
    for name, sha in sources.items():
        archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress(frozen[name], mtime=0))
    result['provenance'] = dict(request_sha256=digest(fixture.read_bytes()), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
    print('Recorded ' + str(output), flush=True)


def main():
    capture(FAMILY, request())


if __name__ == '__main__':
    main()
