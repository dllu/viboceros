"""Verify curvature knot domains and existing MatchSrf behavior without windows."""
from . import check_surface_match as runner
from .merge_surfaces_evidence import ROOT

PROOF=ROOT/'docs/surface-match-domain-proof'
core_commands=runner.commands


def commands():
    for mode in ['portable','native']:
        features=[] if mode=='portable' else ['--features','native-smlib']
        yield 'domain-'+mode,['cargo','test','-p','viboceros-command',*features,
            '--test','surface_match_refinement','--','--nocapture']
        yield 'ui-'+mode,['cargo','test','--bin','viboceros',*features,
            'app::tests::match_surface','--','--nocapture']
    for name,command in core_commands():
        if name=='python':command=[*command,*['tools.rhino_oracle.test_'+module for module in [
            'surface_match_tolerances','surface_match_record','match_surface_menu_record','match_surface_command_record']]]
        yield name,command


if __name__=='__main__':
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
