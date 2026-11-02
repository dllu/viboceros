"""Freeze independent SDK statistics and native Auto Range panel observations."""
import argparse
import gzip
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from .client import OracleClient
from .curvature_auto_sdk_probe import STYLES, validate

HERE = Path(__file__).resolve().parent
FAMILY = 'curvature_auto_sdk'
SAMPLES = HERE / 'fixtures/curvature_auto/samples.json'


def request():
    operations = []
    for style in STYLES:
        for asset in json.loads(SAMPLES.read_text()):
            operations.append(dict(op='curvature_auto_sdk', id=asset+'-'+style, asset=asset, style=style))
        for label, asset, companions in (
                ('empty', None, []), ('pair', 'constant', ['hundred']),
                ('weighted', 'ones100', ['hundred']), ('mixed', 'signed', ['ramp']),
                ('duplicate', 'constant', ['constant', 'constant']),
                ('split103', 'ones49', ['ones54']), ('split106', 'ones103', ['constant']),
                ('duplicate103', 'ones103', ['ones103'])):
            operations.append(dict(op='curvature_auto_sdk', id=label+'-'+style, asset=asset,
                                   companions=companions, style=style))
        for asset in (None, 'constant', 'outlier'):
            operations.append(dict(op='curvature_auto_sdk', id='preserve-'+str(asset)+'-'+style,
                                   asset=asset, style=style, initial=[[-7,11],[2,9],[3,12],[4,13]]))
    for op in operations:
        validate(op)
    return envelope(operations)


def envelope(operations):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-8, relative=1e-12, angular=1e-8), operations=operations)


def panel_request():
    return envelope([
        dict(op='curvature_analysis', id='auto-panel-'+source+'-'+style+('-hidden' if hide else ''),
             source=source, style=style, range=[-1,1] if style in ('Gaussian','Mean') else [0,5],
             preselect=True, steps=['CurvatureAnalysis']+(['Hide'] if hide else [])+['AutoRange']
             +(['Show'] if hide else [])+['CurvatureAnalysisOff'])
        for source in ('box','sphere','warped','cylinder','cone','torus') for style in STYLES
        for hide in ([False,True] if source in ('box','warped') else [False])])


def write_meshes():
    """Build against the existing Cargo OpenNURBS build; verify exact readback."""
    root = HERE.parents[1]
    builds = list((root/'target/debug/build').glob('viboceros-opennurbs-*/out/build/opennurbs/libopennurbsStatic.a'))
    if not builds:
        raise ValueError('run cargo build -p viboceros-opennurbs before generating fixtures')
    build = builds[0].parent
    with tempfile.TemporaryDirectory(prefix='viboceros-curvature-writer-') as directory:
        binary = Path(directory)/'writer'
        subprocess.run(['c++','-std=c++17','-O2','-ffp-contract=off','-I'+str(root/'third_party/opennurbs'),
                        str(HERE/'curvature_samples.cpp'),'-o',str(binary),'-Wl,--start-group',
                        *[str(build/p) for p in ('libopennurbsStatic.a','zlib/libzlib.a',
                           'freetype263/libopennurbs_public_freetype.a','android_uuid/libandroid_uuid.a')],
                        '-Wl,--end-group','-lpthread'], check=True)
        for name, values in json.loads(SAMPLES.read_text()).items():
            path = SAMPLES.parent/(name+'.3dm')
            if path.exists():
                raise ValueError('refusing to overwrite sample mesh '+str(path))
            subprocess.run([str(binary),str(path)], input=''.join('{} {}\n'.format(*p) for p in values),
                           text=True, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--panel', action='store_true')
    parser.add_argument('--write-meshes', action='store_true')
    args = parser.parse_args()
    if args.write_meshes:
        write_meshes()
        return
    family = 'curvature_auto_panel' if args.panel else FAMILY
    query = HERE/'fixtures'/(family+'.json')
    output = HERE/'observations'/(family+'.json.gz')
    if query.exists() or output.exists():
        raise ValueError('curvature captures are immutable')
    q = panel_request() if args.panel else request()
    query.write_text(json.dumps(q, indent=2)+'\n')
    names = ['generate_curvature_auto.py','curvature_auto_sdk_probe.py','curvature_samples.cpp',
             'fixtures/curvature_auto/samples.json','structural_sdk.py','rhino_worker.py','client.py',
             'merge_edges_probe.py']
    names += ['fixtures/curvature_auto/'+name+'.3dm' for name in json.loads(SAMPLES.read_text())]
    if args.panel:
        names += ['curvature_analysis_probe.py','curvature_analysis_controls.py','join_probe.py']
        names += ['fixtures/brep_direction/'+s+'.3dm' for s in ('box','sphere','warped','cylinder','cone','torus')]
    images = {name:(HERE/name).read_bytes() for name in names}
    print('Capturing {} cases'.format(len(q['operations'])), flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleAutoQualified20261102').run_rhino(q, timeout=900)
    if images != {name:(HERE/name).read_bytes() for name in names}:
        raise ValueError('curvature producer changed during capture')
    if len(data['results']) != len(q['operations']) or any('error' in row for row in data['results']):
        raise ValueError('curvature capture failed')
    sources = {name:hashlib.sha256(raw).hexdigest() for name,raw in images.items()}
    data['provenance'] = dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(), sources=sources)
    for name, sha in sources.items():
        path = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():
            path.write_bytes(gzip.compress(images[name], mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data, indent=2, allow_nan=False)+'\n').encode(), mtime=0))
    print('Recorded '+str(output), flush=True)


if __name__ == '__main__':
    main()
