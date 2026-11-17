def book_invoice(invoice_id):
    return f"booked:{invoice_id}"


def renew_subscription(subscription_id):
    return f"renewed:{subscription_id}"


def retry_payment(payment_id):
    return f"retry:{payment_id}"
