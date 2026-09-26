def parse_count(value):
    try:
        return int(value)
    except Exception:
        pass


def public_summary(value):
    return f"count: {value}"
