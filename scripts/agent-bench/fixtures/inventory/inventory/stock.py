"""Stock levels with reservations."""

from __future__ import annotations


class OutOfStock(Exception):
    """Raised when a reservation exceeds the available quantity."""


class Stock:
    def __init__(self) -> None:
        self._on_hand: dict[str, int] = {}
        self._reserved: dict[str, int] = {}

    def receive(self, sku: str, quantity: int) -> None:
        self._on_hand[sku] = self._on_hand.get(sku, 0) + quantity

    def available(self, sku: str) -> int:
        return self._on_hand.get(sku, 0) - self._reserved.get(sku, 0)

    def reserve(self, sku: str, quantity: int) -> None:
        if quantity > self.available(sku):
            raise OutOfStock(sku)
        self._reserved[sku] = self._reserved.get(sku, 0) + quantity

    def release(self, sku: str, quantity: int) -> None:
        held = self._reserved.get(sku, 0)
        self._reserved[sku] = max(0, held - quantity)
