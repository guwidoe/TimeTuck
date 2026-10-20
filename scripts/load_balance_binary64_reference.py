"""UNEXECUTED original binary64 policy reference; no float/libm dependencies.

Fraction/integer midpoint arithmetic models each binary64 conversion/operation,
then sqrt rounded to nearest-even binary64 and six decimal significant digits.
This specifies a native mode; it does not infer Java's implementation.
"""
from fractions import Fraction
from math import isqrt


class NonFiniteEvaluation(ArithmeticError):
    pass


def power2(exponent):
    return Fraction(1 << exponent) if exponent >= 0 else Fraction(1, 1 << -exponent)


def nearest_integer(value):
    assert value >= 0
    quotient, remainder = divmod(value.numerator, value.denominator)
    twice = 2 * remainder
    return quotient + (twice > value.denominator or (twice == value.denominator and quotient % 2))


def exponent2(value):
    assert value > 0
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    return exponent - (value < power2(exponent))


def binary64(value):
    value = Fraction(value)
    if not value:
        return value
    sign = -1 if value < 0 else 1
    value = abs(value)
    exponent = exponent2(value)
    if exponent > 1023:
        raise NonFiniteEvaluation()
    quantum = power2(max(exponent - 52, -1074))
    rounded = nearest_integer(value / quantum) * quantum
    if rounded >= power2(1024):
        raise NonFiniteEvaluation()
    return sign * rounded


def binary64_sqrt(value):
    assert value >= 0
    if not value:
        return value
    quantum = power2(max(exponent2(value) // 2 - 52, -1074))
    squared_units = value / (quantum * quantum)
    a, b = squared_units.numerator, squared_units.denominator
    floor = isqrt(a // b)
    midpoint_squared = b * (2 * floor + 1) ** 2
    if 4 * a > midpoint_squared or (4 * a == midpoint_squared and floor % 2):
        floor += 1
    rounded = floor * quantum
    if rounded >= power2(1024):
        raise NonFiniteEvaluation()
    return rounded


def power10(exponent):
    return Fraction(10 ** exponent) if exponent >= 0 else Fraction(1, 10 ** -exponent)


def six_digits(value):
    assert value >= 0
    if not value:
        return [0, 0]
    exponent = len(str(value.numerator)) - len(str(value.denominator))
    if value < power10(exponent):
        exponent -= 1
    scale = 5 - exponent
    coefficient = nearest_integer(value * power10(scale))
    while coefficient % 10 == 0:
        coefficient //= 10
        scale -= 1
    return [coefficient, scale]


def wrapping_i64(value):
    return (value + 2**63) % 2**64 - 2**63


def evaluate_moments(total, squares, count):
    """Return a six-digit pair or typed error, for supported 64-bit usize counts."""
    if not 0 <= count <= 2**64 - 1:
        raise ValueError('count outside reference target usize64 bounds')
    if not count:
        return [0, 0]
    try:
        # Match the specified operation order, with a rounding after each op.
        q = binary64(squares)
        t = binary64(wrapping_i64(total * total))
        n = binary64(count)
        quotient = binary64(t / n)
        radicand = binary64(q - quotient)
        if radicand < 0:
            return dict(error='NegativeRadicand')
        return six_digits(binary64_sqrt(radicand))
    except NonFiniteEvaluation:
        return dict(error='NonFiniteEvaluation')


def reference(case, slots):
    loads = {}; initials = {}
    for item in slots.values():
        key, load, initial = case['items'][item]
        if key in initials and initials[key] != initial:
            raise ValueError('Conflicting active initial loads')
        initials[key] = initial
        loads[key] = wrapping_i64(loads.get(key, initial) + load)
    total = wrapping_i64(sum(loads.values()))
    squares = wrapping_i64(sum(wrapping_i64(value * value) for value in loads.values()))
    unfairness = evaluate_moments(total, squares, len(loads))
    if isinstance(unfairness, dict):
        return unfairness
    return dict(loads=[[key, load] for key, load in sorted(loads.items())], unfairness=unfairness)
