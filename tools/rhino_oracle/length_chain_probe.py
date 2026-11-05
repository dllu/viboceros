"""Observe the real Length getter's hidden SelChain option."""
if __package__:
    from .chain_selection_probe import run as observe
    from .length_chain_cases import validate
else:
    from chain_selection_probe import run as observe
    from length_chain_cases import validate

def run(op,host,iterations,tolerance):
    return observe(op,host,iterations,tolerance,command='Length')
