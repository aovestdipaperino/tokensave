from pricing import invoice_total


def checkout(prices, percent):
    return invoice_total(prices, percent)


def preview(prices, percent):
    return invoice_total(prices, percent)
