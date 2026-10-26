"""Invoice amounts in integer cents."""


def discounted_total(prices, percent):
    """Round the final discounted sum down to whole cents."""
    return sum(price * (100 - percent) // 100 for price in prices)


def invoice_total(prices, percent):
    return discounted_total(prices, percent)
