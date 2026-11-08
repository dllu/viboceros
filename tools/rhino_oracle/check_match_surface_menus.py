"""Check owned MatchSrf enum choices in portable and SMLib builds without windows."""
import argparse,gzip,json,subprocess,time
from .match_surface_menu_evidence import ROOT,PROOF


def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::match_surface','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']
    yield 'python',['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in ['match_surface_menus','match_surface_command','match_surface_command_record','surface_match_record','structural_sdk','client']]]


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--resume',action='store_true');args=parser.parse_args()
    expected=list(commands());inventory=dict(expected)
    saved=json.loads((PROOF/'checks.json').read_bytes()) if args.resume else [];done={c['name']:c for c in saved}
    if len(done)!=len(saved) or any(c['name'] not in inventory or c['command']!=inventory[c['name']] for c in saved):raise ValueError('saved menu check inventory')
    for name,command in expected:
        if name in done and done[name]['status']==0:continue
        if name=='python-subd':
            from . import check_blend_curve_empty as runner
            runner.PROOF=PROOF;runner.refresh_subd()
        print('Checking '+name,flush=True);start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0));done[name]=dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start)
        (PROOF/'checks.json').write_text(json.dumps([done[n]for n,_ in expected if n in done],indent=2)+'\n')
        if result.returncode:print(result.stdout.decode(errors='replace')[-5000:]);raise SystemExit(result.returncode)
    print('All MatchSrf menu checks passed',flush=True)


if __name__=='__main__':main()
