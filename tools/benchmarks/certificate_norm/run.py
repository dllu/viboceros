"""Compare complete certified surface edits against a frozen geometry revision.

Both implementations receive the same authored source and native fitted support.
Only the copied baseline crate is renamed; the working tree is read-only.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile

ROOT=Path(__file__).resolve().parents[3]
BASELINE='a7bc7da4c3ec667ea405c4ab6acad25c65037d2c'
GEOMETRY='crates/viboceros-geometry'
SOURCE=[GEOMETRY+'/src/surface_pullback/certificate/'+p for p in ['algebra.rs','interval.rs']]
DRIVER=Path(__file__).resolve().with_name('driver.rs')

def run():
    archive=subprocess.check_output(['git','archive',BASELINE,GEOMETRY],cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix='viboceros-certificate-bench-') as directory:
        work=Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as source:
            source.extractall(work,filter='data')
        manifest=work/GEOMETRY/'Cargo.toml'
        manifest.write_text(manifest.read_text().replace('name = "viboceros-geometry"',
            'name = "viboceros-geometry-certificate-baseline"'))
        for name in ['docs','tools','third_party']:
            (work/name).symlink_to(ROOT/name,target_is_directory=True)
        text=(ROOT/'Cargo.toml').read_text()
        dependencies=re.search(r'(?ms)^\[workspace.dependencies\]\n(.*?)(?=^\[|\Z)',text)[1]
        dependencies=dependencies.replace('path = "crates/','path = "'+str(ROOT/'crates')+'/')
        q=lambda p:json.dumps(str(p))
        (work/'Cargo.toml').write_text('''[package]
name = "viboceros-certificate-benchmark"
version = "0.0.0"
edition = "2024"
[workspace]
members = ["crates/viboceros-geometry"]
[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.95"
[workspace.dependencies]
'''+dependencies+'''
[dependencies]
baseline_geo = {package = "viboceros-geometry-certificate-baseline",path = "crates/viboceros-geometry"}
current_geo = {package = "viboceros-geometry",path = '''+q(ROOT/GEOMETRY)+'''}
viboceros-smlib = {path = '''+q(ROOT/'crates/viboceros-smlib')+''',features = ["native"]}
''')
        shutil.copyfile(ROOT/'Cargo.lock',work/'Cargo.lock')
        (work/'src').mkdir();shutil.copyfile(DRIVER,work/'src/main.rs')
        env=dict(os.environ,VIBOCEROS_CERTIFICATE_OUTPUT=str(work),
            CARGO_TARGET_DIR=str(ROOT/'target'))
        result=subprocess.run(['cargo','run','--offline','--manifest-path',str(work/'Cargo.toml')],
            cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        print(result.stdout.decode(errors='replace'),end='',flush=True)
        result.check_returncode()
        output=result.stdout.decode();cases=[]
        for case,distance,nu,nv in re.findall(r'^CERTIFICATE_CASE (\d+) ([-.\d]+) (\d+) (\d+)$',output,re.M):
            times={mode:float(seconds) for mode,seconds in re.findall(
                r'^CERTIFICATE_TIME '+case+r' (baseline|candidate) ([.\d]+)$',output,re.M)}
            before=(work/('baseline-'+case+'.txt')).read_bytes()
            after=(work/('candidate-'+case+'.txt')).read_bytes()
            if before!=after or set(times)!={'baseline','candidate'}:raise ValueError('incomplete benchmark or changed geometry')
            cases.append(dict(case=int(case),distance=float(distance),support_counts=[int(nu),int(nv)],
                baseline_seconds=times['baseline'],candidate_seconds=times['candidate'],
                baseline_geometry_sha256=hashlib.sha256(before).hexdigest(),
                candidate_geometry_sha256=hashlib.sha256(after).hexdigest()))
        if [c['case'] for c in cases]!=[0,1]:raise ValueError('benchmark inventory')
        return dict(baseline_revision=BASELINE,cases=cases,
            current_source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in SOURCE},
            producer_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [Path(__file__).resolve(),DRIVER]})

if __name__=='__main__':
    print('CERTIFICATE_NORM_BENCHMARK '+json.dumps(run(),sort_keys=True,allow_nan=False))
