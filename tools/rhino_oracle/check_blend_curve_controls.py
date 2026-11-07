"""Higher blend controls and affected windowless/kernel regressions."""
from .check_blend_curve_alt import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-controls-proof'
def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'controls-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_controls','--','--nocapture']
        yield 'controls-ui-'+mode,['cargo','test','--bin','viboceros',*features,'app::tests::blend_curve_controls','--','--nocapture']
    yield 'geometry-kernel',['cargo','test','-p','viboceros-geometry','--lib','--','--nocapture']
    for name,command in prior_commands():
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_controls']
        yield name,command
def main():
    from . import check_blend_curve_points as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
if __name__=='__main__':main()
