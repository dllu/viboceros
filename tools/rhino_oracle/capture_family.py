"""Record an immutable native fixture with frozen, content-addressed producers."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .capture_preflight import CLIENT_SOURCE_FILES

HERE = Path(__file__).resolve().parent


def capture(family, query, sources, scheme, timeout=1200):
    fixture = HERE / ('fixtures/' + family + '.json')
    output = HERE / ('observations/' + family + '.json')
    if output.exists():
        raise ValueError('successful native captures are immutable: ' + family)
    encoded = (json.dumps(query, indent=2, allow_nan=False) + '\n').encode()
    if fixture.exists() and fixture.read_bytes() != encoded:
        raise ValueError('existing fixture differs: ' + family)
    fixture.write_bytes(encoded)
    if any(name in CLIENT_SOURCE_FILES for name in sources):
        sources = [*sources, *CLIENT_SOURCE_FILES]
    sources = list(dict.fromkeys([*sources, 'capture_family.py']))
    frozen = {name: (HERE / name).read_bytes() for name in sources}
    digest = lambda data: hashlib.sha256(data).hexdigest()
    print('Capturing {} cases: {}'.format(len(query['operations']), family), flush=True)
    data = OracleClient(settings_scheme=scheme).run_rhino(query, timeout=timeout)
    for name, original in frozen.items():
        if (HERE / name).read_bytes() != original:
            raise ValueError('producer changed during capture: ' + name)
    if fixture.read_bytes() != encoded:
        raise ValueError('fixture changed during capture: ' + family)
    if [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]:
        raise ValueError('native coverage differs: ' + family)
    data['provenance'] = dict(request_sha256=digest(encoded),
                              sources={name: digest(content) for name, content in frozen.items()})
    for content in frozen.values():
        archive = HERE / ('observations/source_snapshots/' + digest(content) + '.gz')
        if archive.exists():
            if gzip.decompress(archive.read_bytes()) != content:
                raise ValueError('source archive differs: ' + str(archive))
        else:
            with archive.open('xb') as stream:
                stream.write(gzip.compress(content, mtime=0))
    with output.open('x') as stream:
        stream.write(json.dumps(data, indent=2, allow_nan=False) + '\n')
    print('Recorded ' + family, flush=True)
