"""Orders: lines, totals and stock reservation."""

from __future__ import annotations

from dataclasses import dataclass, field

from inventory.money import ZERO, Money
from inventory.pricing import line_total
from inventory.stock import Stock


@dataclass(frozen=True)
class OrderLine:
    sku: str
    unit_price: Money
    quantity: int

    def total(self) -> Money:
        return line_total(self.unit_price, self.quantity)


@dataclass
class Order:
    lines: list[OrderLine] = field(default_factory=list)

    def add(self, sku: str, unit_price: Money, quantity: int) -> None:
        self.lines.append(OrderLine(sku, unit_price, quantity))

    def total(self) -> Money:
        total = ZERO
        for line in self.lines:
            total = total + line.total()
        return total

    def reserve(self, stock: Stock) -> None:
        """Reserve every line, or nothing if any line is out of stock."""
        done: list[OrderLine] = []
        try:
            for line in self.lines:
                stock.reserve(line.sku, line.quantity)
                done.append(line)
        except Exception:
            for line in done:
                stock.release(line.sku, line.quantity)
            raise
