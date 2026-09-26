def page_slice(items: list[str], page: int, page_size: int) -> list[str]:
    """Return one-based page contents."""
    if page < 1 or page_size < 1:
        raise ValueError("page and page_size must be positive")
    start = page * page_size
    return items[start : start + page_size]
