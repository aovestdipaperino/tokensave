class PyThing:
    value: int


def py_target(item: PyThing) -> int:
    return item.value


def first_py_call() -> int:
    return py_target(PyThing())


def second_py_call(item: PyThing) -> int:
    return py_target(item)
