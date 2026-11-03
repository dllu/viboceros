"""Freeze public load/save behavior, including the zero-density fallback trigger."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from .capture_family import HERE,capture
from .analysis_profile_probe import ARCHIVES,IDENTITY_ARCHIVES,validate


def request(identity=False):
    operations=[dict(op='analysis_profile_identity' if identity else 'analysis_profile',id=name,archive=name)
                for name in (IDENTITY_ARCHIVES if identity else ARCHIVES)]
    for op in operations:validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def write_inputs():
    root=HERE.parents[1]
    builds=list((root/'target/debug/build').glob('viboceros-opennurbs-*/out/build/opennurbs/libopennurbsStatic.a'))
    if not builds:raise ValueError('build viboceros-opennurbs before generating profile inputs')
    build=builds[0].parent
    with tempfile.TemporaryDirectory(prefix='viboceros-analysis-profiles-') as directory:
        binary=Path(directory)/'writer'
        subprocess.run(['c++','-std=c++17','-O2','-ffp-contract=off','-I'+str(root/'third_party/opennurbs'),
            str(HERE/'analysis_profile_fixtures.cpp'),'-o',str(binary),'-Wl,--start-group',
            *[str(build/p) for p in ('libopennurbsStatic.a','zlib/libzlib.a',
                'freetype263/libopennurbs_public_freetype.a','android_uuid/libandroid_uuid.a')],
            '-Wl,--end-group','-lpthread'],check=True)
        subprocess.run([str(binary),str(HERE/'fixtures/analysis_profiles')],check=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write-inputs',action='store_true')
    parser.add_argument('--identity',action='store_true');args=parser.parse_args()
    if args.write_inputs:write_inputs();return
    capture('analysis_profile_identity' if args.identity else 'analysis_profile',request(args.identity),['generate_analysis_profile.py','analysis_profile_probe.py',
        'analysis_profile_fixtures.cpp','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py',
        *['fixtures/analysis_profiles/'+name+'.3dm' for name in (IDENTITY_ARCHIVES if args.identity else ARCHIVES)]],
        'VibocerosOracleAnalysisProfileIdentity20261102' if args.identity else 'VibocerosOracleAnalysisProfile20261102',timeout=900)


if __name__=='__main__':main()
