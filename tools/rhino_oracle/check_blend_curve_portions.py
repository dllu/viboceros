"""Temporary-source BlendCrv workflows and inherited windowless regressions."""
from .check_blend_curve_points import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-portions-proof'
def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'portion-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_portions','--','--nocapture']
        yield 'portion-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_portions','--','--nocapture']
    for name,command in prior_commands():
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_portions']
        yield name,command

def main():
    # Keep the established runner and its guarded SubD hash refresh policy.
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF
    runner.commands=commands
    runner.main()
if __name__=='__main__':main()
