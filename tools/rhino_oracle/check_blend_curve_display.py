"""Windowless curvature display, continuation defaults and affected regressions."""
from .check_blend_curve_empty import ROOT,commands as prior_commands
PROOF=ROOT/'docs/blend-curve-display-proof'

def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'display-command-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','blend_curve_display','--','--nocapture']
        yield 'graph-library-'+mode,['cargo','test','-p','viboceros-command',*features,'--lib','curvature_graph','--','--nocapture']
        yield 'graph-viewport-'+mode,['cargo','test','--bin','viboceros',*features,'viewport::curvature_graph','--','--nocapture']
    for name,command in prior_commands():
        if name.startswith('tolerance-library-'):continue
        if name.startswith('empty-ui-'):name=name.replace('empty-ui-','display-ui-')
        if name=='python':command=[*command,'tools.rhino_oracle.test_blend_curve_display']
        yield name,command

def main():
    from . import check_blend_curve_empty as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()

if __name__=='__main__':main()
