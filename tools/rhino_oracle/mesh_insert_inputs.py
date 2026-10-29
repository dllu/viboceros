"""Export immutable source 3DM inputs from a retained native capture."""
import base64,json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent

def save_sources(family):
    if not re.fullmatch(r'[a-z_]+',family):raise ValueError('owned mesh insertion family required')
    record=json.loads((HERE/'observations'/(family+'.json')).read_text())
    folder=HERE/'fixtures'/family;folder.mkdir(parents=True,exist_ok=True)
    for row in record['results']:
        name=row['id']
        if not re.fullmatch(r'[a-z0-9_-]+',name):raise ValueError('owned mesh insertion ID required')
        data=base64.b64decode(row['value']['source_3dm'],validate=True)
        if not data.startswith(b'3D Geometry File Format'):raise ValueError('native source archive required')
        path=folder/(name+'.3dm')
        if path.exists():
            if path.read_bytes()!=data:raise ValueError('existing native source differs: '+name)
        else:
            with path.open('xb')as stream:stream.write(data)
