"""Independent exact Fraction coefficients and high-precision Decimal roots."""
from decimal import Decimal, localcontext
from fractions import Fraction
import json
import math
import random


def reference(distance, first, second):
    d, r, s = map(Fraction, (distance, first, second))
    along = (d*d + r*r - s*s) / (2*d)
    square = r*r - along*along
    assert square > 0
    with localcontext() as context:
        context.prec = 180
        height = float((Decimal(square.numerator) / Decimal(square.denominator)).sqrt())
    return dict(distance=distance, first_radius=first, second_radius=second,
                along=float(along), height=height)


rng = random.Random(8102026)
rows = []
for i in range(160):
    r, s = rng.randint(2, 1000), rng.randint(2, 1000)
    d = rng.randint(abs(r-s)+1, r+s-1)
    scale = math.ldexp(1., [-550, -250, 0, 250, 600][i % 5])
    rows.append(reference(d*scale, r*scale, s*scale))
for exponent in [-500, 0, 500]:
    scale = math.ldexp(1., exponent)
    for power in [20, 26, 40, 50]:
        r = math.ldexp(1., power)
        rows.append(reference((1.+2.**-20)*scale, r*scale, (r-1.)*scale))
print(json.dumps(dict(generator='tools/numerics/generate_circle_cut_reference.py', cases=rows), indent=2, allow_nan=False))
