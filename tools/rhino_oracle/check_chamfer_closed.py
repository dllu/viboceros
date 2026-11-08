"""Windowless closed-source Chamfer, shared admission and protocol checks."""
from .check_blend_curve_empty import ROOT
PROOF=ROOT/'docs/chamfer-closed-proof'
def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'closed-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','chamfer_closed','--','--nocapture']
        yield 'closed-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::chamfer_closed','--','--nocapture']
        suites=['chamfer_getter','chamfer_self','fillet_getter','fillet_closed','fillet_self']
        yield 'command-regressions-'+mode,['cargo','test','-p','viboceros-command',*features,*[arg for s in suites for arg in ['--test',s]],'--','--nocapture']
        for label,suite in [('chamfer-ui','curve_chamfer'),('self-ui','chamfer_self'),('fillet-ui','fillet')]:
            yield label+'-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::'+suite if suite!='fillet' else suite,'--','--nocapture']
        yield 'library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib','fillet::tests','--','--nocapture']
        yield 'clippy-'+mode,['cargo','clippy','--workspace','--all-targets',*features,'--','-D','warnings']
    yield 'fmt',['cargo','fmt','--all','--check']
    yield 'python',['python3','-m','unittest','tools.rhino_oracle.test_structural_sdk','tools.rhino_oracle.test_client','tools.rhino_oracle.test_chamfer_getter','tools.rhino_oracle.test_chamfer_self','tools.rhino_oracle.test_chamfer_closed']
    yield 'subd-command-native',['cargo','test','-p','viboceros-command','--features','native-smlib','--test','subd_box_native','--test','subd_plane_native','--','--nocapture']
    yield 'subd-ui-native',['cargo','test','--bin','viboceros','--features','native-smlib','subd_primitives','--','--nocapture']
    yield 'python-subd',['python3','-m','unittest','tools.rhino_oracle.test_subd_box','tools.rhino_oracle.test_subd_plane']
def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()
