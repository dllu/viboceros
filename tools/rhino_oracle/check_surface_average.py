"""Check paired averaging, command history and affected regressions headlessly."""
from . import check_surface_match as runner
from .check_surface_match_domain import commands as regressions
from .surface_average_evidence import ROOT,PROOF


def commands():
    for mode in ['portable','native']:
        features=[]if mode=='portable'else['--features','native-smlib']
        yield 'average-'+mode,['cargo','test','-p','viboceros-command',*features,'--test','average_surfaces','--','--nocapture']
        yield 'command-'+mode,['cargo','test','-p','viboceros-command',*features,
            '--test','match_surface_average_command','--test','match_surface_command','--','--nocapture']
    for name,command in regressions():
        if name=='python':command=[*command,'tools.rhino_oracle.test_match_surface_average','tools.rhino_oracle.test_surface_average']
        yield name,command


if __name__=='__main__':
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
