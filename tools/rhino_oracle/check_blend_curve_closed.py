"""Whole closed BlendCrv sources and affected windowless command regressions."""
from .check_blend_curve_controls import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-closed-proof'
def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'closed-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_closed','--','--nocapture']
        yield 'closed-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_closed','--','--nocapture']
    for name,command in prior_commands():
        if name=='geometry-kernel':continue # Geometry algorithms are unchanged.
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_closed']
        yield name,command
def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()
