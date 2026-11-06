"""Windowless point-grid input checks and shared command regressions."""
import argparse,gzip,json,subprocess,time
from .check_srf_pt import ROOT
PROOF=ROOT/'docs/point-grid-input-proof'

def commands():
    yield 'fmt',['cargo','fmt','--all','--','--check']
    yield 'command-unit',['cargo','test','-p','viboceros-command','--lib','point_grid','--','--nocapture']
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'commands-'+mode,['cargo','test','-p','viboceros-command',*features,
            *sum((['--test',n] for n in ['srf_pt','plane_surface','plane_surface_native','plane_completion','subd_box_native','subd_plane_native']),[]),'--','--nocapture']
        for name,pattern in [('ui','app::tests::surface_point_grid'),('corners','app::tests::srf_pt'),('plane','app::tests::plane_')]:
            yield name+'-'+mode,['cargo','test','--bin','viboceros',*features,pattern,'--','--nocapture']
    yield 'subd-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'clippy-portable',['cargo','clippy','--workspace','--all-targets','--','-D','warnings']
    yield 'clippy-all-features',['cargo','clippy','--workspace','--all-targets','--all-features','--','-D','warnings']
    yield 'python',['python3','-m','unittest',*('tools.rhino_oracle.test_'+n for n in ['point_grid_input','srf_pt','srf_pt_followup','plane_surface','plane_completion','rectangle','structural_sdk','client','subd_box','subd_plane'])]

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--resume',action='store_true');args=parser.parse_args()
    PROOF.mkdir(exist_ok=True);checks=json.loads((PROOF/'checks.json').read_text()) if args.resume else []
    expected=list(commands())
    if [(c['name'],c['command']) for c in checks]!=expected[:len(checks)]:raise ValueError('saved check commands changed')
    checks=checks[:next((i for i,c in enumerate(checks) if c['status']),len(checks))]
    for name,command in expected[len(checks):]:
        if name=='python':
            from . import report_subd_box,report_subd_plane
            text='\n'.join(gzip.decompress((PROOF/(n+'.log.gz')).read_bytes()).decode() for n in ['commands-native','subd-native'])
            for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
                path=ROOT/('docs/subd-'+kind+'-qualification.json');before=json.loads(path.read_text());after=module.summarize(text)
                if {k:v for k,v in before.items() if k!='sha256'}!={k:v for k,v in after.items() if k!='sha256'} or set(before['sha256'])!=set(after['sha256']):raise ValueError('shared SubD behavior or inventory changed')
                after['sha256']={p:after['sha256'][p] for p in before['sha256']};path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')
        print('Checking '+name,flush=True);start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        checks.append(dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:]);raise SystemExit(result.returncode)
    print('All point-grid input checks passed',flush=True)
if __name__=='__main__':main()
