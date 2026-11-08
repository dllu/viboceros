"""Run focused surface matching and affected adapter checks without windows."""
import argparse,gzip,json,subprocess,time
from .surface_match_evidence import ROOT,PROOF


def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'surface-'+mode,['cargo','test','-p','viboceros-command',*features,
            '--test','match_surfaces','--test','merge_surfaces','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'kernel-native',['cargo','test','-p','viboceros-smlib','--features','native',
        '--test','surface_merge','--test','sheets','--','--nocapture']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib',
        '--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']
    yield 'python',['python3','-m','unittest',*['tools.rhino_oracle.test_'+name for name in [
        'surface_match','merge_surfaces','merge_surface_command','merge_surface_command_record',
        'client','capture_family','structural_sdk']]]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resume',action='store_true')
    parser.add_argument('--rerun',action='append',default=[])
    args=parser.parse_args();PROOF.mkdir(exist_ok=True)
    expected=list(commands());inventory=dict(expected)
    if set(args.rerun)-inventory.keys():parser.error('unknown check requested for rerun')
    saved=json.loads((PROOF/'checks.json').read_bytes()) if args.resume else []
    done={c['name']:c for c in saved}
    if len(done)!=len(saved) or any(c['name'] not in inventory or c['command']!=inventory[c['name']] for c in saved):
        raise ValueError('saved check inventory changed')
    for name,command in expected:
        if name not in args.rerun and name in done and done[name]['status']==0:continue
        if name=='python-subd':
            from . import check_blend_curve_empty as runner
            runner.PROOF=PROOF;runner.refresh_subd()
        print('Checking '+name,flush=True);start=time.monotonic()
        result=subprocess.run(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (PROOF/(name+'.log.gz')).write_bytes(gzip.compress(result.stdout,mtime=0))
        done[name]=dict(name=name,command=command,status=result.returncode,seconds=time.monotonic()-start)
        (PROOF/'checks.json').write_text(json.dumps([done[n] for n,_ in expected if n in done],indent=2)+'\n')
        if result.returncode:
            print(result.stdout.decode(errors='replace')[-6000:]);raise SystemExit(result.returncode)
    print('All surface-match checks passed',flush=True)


if __name__=='__main__':main()
