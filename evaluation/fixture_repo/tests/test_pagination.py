from src.pagination import page_slice


def test_page_one_returns_first_page() -> None:
    assert page_slice(["a", "b", "c", "d"], page=1, page_size=2) == ["a", "b"]


def test_page_two_returns_second_page() -> None:
    assert page_slice(["a", "b", "c", "d"], page=2, page_size=2) == ["c", "d"]
