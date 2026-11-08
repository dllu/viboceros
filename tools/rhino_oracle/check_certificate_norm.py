"""Headless certificate arithmetic and affected offset regressions."""
from .check_offset_srf_trims import ROOT,commands as trim_commands
PROOF=ROOT/'docs/certificate-norm-proof'

def commands():
    for name,command in trim_commands():
        yield name,command+['tools.rhino_oracle.test_certificate_norm'] if name=='python' else command
        if name=='geometry':
            yield 'benchmark',['python3','tools/benchmarks/certificate_norm/run.py']

if __name__=='__main__':
    from . import check_offset_srf_trims as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
