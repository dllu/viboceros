"""Run G2 averaging and affected headless regression groups separately."""
from . import check_surface_match as runner
from .check_surface_average import commands as previous
from .surface_average_curvature_evidence import PROOF


def commands():
    for name,command in previous():
        if name=='python':command=[*command,'tools.rhino_oracle.test_surface_average_curvature']
        yield name,command


if __name__=='__main__':
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
