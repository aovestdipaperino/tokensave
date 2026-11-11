"""Integer-cent money arithmetic."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Money:
    """An amount in cents. Never a float, so totals are exact."""

    cents: int

    @classmethod
    def from_units(cls, units: int, cents: int = 0) -> "Money":
        return cls(units * 100 + cents)

    def __add__(self, other: "Money") -> "Money":
        return Money(self.cents + other.cents)

    def times(self, quantity: int) -> "Money":
        return Money(self.cents * quantity)

    def percent_off(self, percent: int) -> "Money":
        """Reduce by `percent`, rounding the discount down to whole cents."""
        discount = self.cents * percent // 100
        return Money(self.cents - discount)

    def __str__(self) -> str:
        return f"{self.cents // 100}.{self.cents % 100:02d}"


ZERO = Money(0)
