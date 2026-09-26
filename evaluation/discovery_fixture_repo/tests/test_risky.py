from src.risky import parse_count


def test_parse_count():
    assert parse_count("3") == 3
