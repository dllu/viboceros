"""Windowless corner-surface qualification and shared-surface regressions."""
import gzip
import json
from pathlib import Path
import subprocess
import time

ROOT=Path(__file__).resolve().parents[2]
PROOF=ROOT/'docs/srf-pt-proof'

def commands():
    yield 'fmt',['cargo','fmt','--all','--','--check']
    yield 'geometry',['cargo','test','-p','viboceros-geometry','--lib','brep::rectangular_surface','--','--nocapture']
    yield 'creation',['cargo','test','-p','viboceros-command','--lib','tests::surfaces::creation','--','--nocapture']
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'commands-'+mode,['cargo','test','-p','viboceros-command',*features,
            *sum((['--test',n] for n in ['srf_pt','plane_surface','plane_surface_native','plane_completion','subd_box_native','subd_plane_native']),[]),'--','--nocapture']
        for kind,pattern in [('ui','app::tests::srf_pt'),('plane','app::tests::plane_'),('face-input','face_edge_input'),('primitive-prompts','interface::primitive_prompts'),('subd','subd_primitives'),('surface-flow','surface_flow')]:
            yield kind+'-'+mode,['cargo','test','-p','viboceros',*features,'--bin','viboceros',pattern,'--','--nocapture']
    yield 'clippy-portable',['cargo','clippy','--workspace','--all-targets','--','-D','warnings']
    yield 'clippy-all-features',['cargo','clippy','--workspace','--all-targets','--all-features','--','-D','warnings']
    yield 'python',['python3','-m','unittest',*('tools.rhino_oracle.test_'+n for n in ['srf_pt','plane_surface','plane_completion','rectangle','structural_sdk','client','subd_box','subd_plane'])]

def main():
    PROOF.mkdir(exist_ok=True);checks=[]
    for name,command in commands():
        if name=='python':
            from . import report_subd_box,report_subd_plane
            text='\n'.join(gzip.decompress((PROOF/(n+'.log.gz')).read_bytes()).decode() for n in ['commands-native','subd-native'])
            for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
                path=ROOT/('docs/subd-'+kind+'-qualification.json');before=json.loads(path.read_text());after=module.summarize(text)
                if {k:v for k,v in before.items() if k!='sha256'}!={k:v for k,v in after.items() if k!='sha256'}:raise ValueError('shared SubD behavior changed')
                if set(before['sha256'])!=set(after['sha256']):raise ValueError('shared source inventory changed')
                after['sha256']={p:after['sha256'][p] for p in before['sha256']};path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')
        print('Checking '+name,flush=True);start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        checks.append(dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-5000:]);raise SystemExit(result.returncode)
    print('All corner-surface checks passed',flush=True)
if __name__=='__main__':main()
