import unittest

from inventory.money import Money


class MoneyTest(unittest.TestCase):
    def test_percent_off_rounds_discount_down(self):
        self.assertEqual(Money(999).percent_off(5), Money(950))

    def test_str(self):
        self.assertEqual(str(Money.from_units(3, 7)), "3.07")


if __name__ == "__main__":
    unittest.main()
