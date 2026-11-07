"""Surface-edge BlendCrv and inherited windowless regressions."""
from .check_blend_curve_symmetry import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-edges-proof'
def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'edge-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_edges','--','--nocapture']
        yield 'edge-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_edges','--','--nocapture']
    for name,command in prior_commands():
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_edges']
        yield name,command
def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()
