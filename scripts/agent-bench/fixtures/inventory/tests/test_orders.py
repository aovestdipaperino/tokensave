import unittest

from inventory import Money, Order, OutOfStock, Stock


class OrderTest(unittest.TestCase):
    def test_small_order_has_no_discount(self):
        order = Order()
        order.add("bolt", Money(250), 4)
        self.assertEqual(order.total(), Money(1000))

    def test_ten_units_get_the_bulk_discount(self):
        order = Order()
        order.add("bolt", Money(200), 10)
        self.assertEqual(order.total(), Money(1900))

    def test_large_bulk_discount(self):
        order = Order()
        order.add("nut", Money(10), 100)
        self.assertEqual(order.total(), Money(880))

    def test_reserve_is_all_or_nothing(self):
        stock = Stock()
        stock.receive("bolt", 5)
        stock.receive("nut", 1)
        order = Order()
        order.add("bolt", Money(1), 5)
        order.add("nut", Money(1), 2)
        with self.assertRaises(OutOfStock):
            order.reserve(stock)
        self.assertEqual(stock.available("bolt"), 5)


if __name__ == "__main__":
    unittest.main()
