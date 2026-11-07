"""Shift-paired BlendCrv editing and inherited windowless regressions."""
from .check_blend_curve_portions import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-symmetry-proof'
def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'symmetry-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_symmetry','--','--nocapture']
        yield 'symmetry-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_symmetry','--','--nocapture']
    for name,command in prior_commands():
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_symmetry','tools.rhino_oracle.test_catenary_point_picker']
        yield name,command

def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()
