"""Observe complete original source pieces participating in each planar loop."""
if __package__:
    from .planar_chains_cases import validate
    from .planar_regions_probe import run as observe
else:
    from planar_chains_cases import validate
    from planar_regions_probe import run as observe


def run(op,host,iterations,tolerance):
    return observe(op,host,iterations,tolerance,validator=validate,subset=True)
