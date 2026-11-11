import sys


def test_python_meets_requires_python():
    assert sys.version_info >= (3, 13)
