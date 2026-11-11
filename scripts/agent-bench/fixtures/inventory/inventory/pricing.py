"""Line pricing rules.

Bulk discounts: a line of BULK_THRESHOLD or more units gets BULK_PERCENT off;
a line of LARGE_BULK_THRESHOLD or more units gets LARGE_BULK_PERCENT off.
"""

from inventory.money import Money

BULK_THRESHOLD = 10
BULK_PERCENT = 5
LARGE_BULK_THRESHOLD = 100
LARGE_BULK_PERCENT = 12


def bulk_discount_percent(quantity: int) -> int:
    """Percentage off for a line of `quantity` units."""
    if quantity >= LARGE_BULK_THRESHOLD:
        return LARGE_BULK_PERCENT
    if quantity > BULK_THRESHOLD:
        return BULK_PERCENT
    return 0


def line_total(unit_price: Money, quantity: int) -> Money:
    """Price of `quantity` units after the bulk discount."""
    if quantity < 0:
        raise ValueError("quantity must not be negative")
    gross = unit_price.times(quantity)
    return gross.percent_off(bulk_discount_percent(quantity))
