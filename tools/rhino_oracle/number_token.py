"""Bounded round-trip decimal tokens without the host's float formatter.

The legacy Rhino Python host can round a double before honoring .17g/.17e.
Integer arithmetic on its IEEE bits keeps the same 17 significant digits on
both Python hosts. This code formats numbers; it never formats script text.
"""
import math
import struct


def number_token(value):
    value = float(value)
    if math.isnan(value) or math.isinf(value):
        raise ValueError("macro numbers must be finite")
    bits = struct.unpack("!Q", struct.pack("!d", value))[0]
    sign = "-" if bits >> 63 else ""
    exponent = (bits >> 52) & 2047
    mantissa = bits & ((1 << 52) - 1)
    if exponent:
        mantissa += 1 << 52
        exponent = exponent - 1023 - 52
    else:
        exponent = -1074
    if not mantissa:
        return sign + "0"
    if exponent < 0:
        integer = mantissa * 5 ** (-exponent)
        decimal_exponent = exponent
    else:
        integer = mantissa << exponent
        decimal_exponent = 0
    discard = max(0, len(str(integer)) - 17)
    if discard:
        divisor = 10 ** discard
        rounded, remainder = divmod(integer, divisor)
        if 2 * remainder > divisor or (
            2 * remainder == divisor and rounded % 2
        ):
            rounded += 1
        integer = rounded
        decimal_exponent += discard
    # At most 18 digits, including a rounding carry, and a short signed exponent.
    token = sign + str(integer) + "e" + str(decimal_exponent)
    if float(token) != value:
        raise ValueError("macro number lost precision")
    return token
