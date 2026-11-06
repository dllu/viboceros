"""Windowless Line construction qualification and shared getter regressions."""
import argparse,gzip,json,subprocess,time
from .check_lines import ROOT,commands as prior_commands
PROOF=ROOT/'docs/line-construction-proof'
def commands():
    yield 'fmt',['cargo','fmt','--all','--','--check']
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'line-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','line_construction','--','--nocapture']
        yield 'line-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::line_construction','--','--nocapture']
        yield 'primitive-input-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::primitive_input','--','--nocapture']
        yield 'rectangle-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::rectangle','--','--nocapture']
    for name,cmd in prior_commands():
        if name=='fmt':continue
        if name=='python':cmd=[*cmd,'tools.rhino_oracle.test_line_construction']
        yield name,cmd

def refresh_subd():
    from . import report_subd_box,report_subd_plane
    text='\n'.join(gzip.decompress((PROOF/(n+'.log.gz')).read_bytes()).decode() for n in ['commands-native','subd-native'])
    for kind,module in [('box',report_subd_box),('plane',report_subd_plane)]:
        path=ROOT/('docs/subd-'+kind+'-qualification.json');before=json.loads(path.read_text());after=module.summarize(text)
        if {k:v for k,v in before.items() if k!='sha256'}!={k:v for k,v in after.items() if k!='sha256'} or set(before['sha256'])!=set(after['sha256']):raise ValueError('shared SubD behavior or inventory changed')
        after['sha256']={p:after['sha256'][p] for p in before['sha256']};path.write_text(json.dumps(after,indent=2,allow_nan=False)+'\n')

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--resume',action='store_true');args=p.parse_args()
    PROOF.mkdir(exist_ok=True);checks=json.loads((PROOF/'checks.json').read_text()) if args.resume else []
    expected=list(commands())
    if [(c['name'],c['command']) for c in checks]!=expected[:len(checks)]:raise ValueError('saved check commands changed')
    checks=checks[:next((i for i,c in enumerate(checks) if c['status']),len(checks))]
    for name,cmd in expected[len(checks):]:
        if name=='python':refresh_subd()
        print('Checking '+name,flush=True);start=time.monotonic();result=subprocess.run(cmd,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0));checks.append(dict(name=name,command=cmd,status=result.returncode,seconds=time.monotonic()-start))
        (PROOF/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if result.returncode:print(result.stdout.decode(errors='replace')[-6000:]);raise SystemExit(result.returncode)
    print('All Line construction checks passed',flush=True)
if __name__=='__main__':main()
