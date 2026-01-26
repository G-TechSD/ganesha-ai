def my_range(start, stop=None, step=1):
    """
    A simple range function.

    Args:
        start: The starting value (inclusive).
        stop: The ending value (exclusive). If None, start is used as stop and start defaults to 0.
        step: The increment/decrement value.

    Returns:
        A list of numbers in the specified range.
    """
    if stop is None:
        stop = start
        start = 0

    if step == 0:
        raise ValueError("Step cannot be zero.")

    result = []
    if step > 0:
        while start < stop:
            result.append(start)
            start += step
    else:
        while start > stop:
            result.append(start)
            start += step

    return result

if __name__ == '__main__':
    # Example Usage
    print(my_range(5))  # Output: [0, 1, 2, 3, 4]
    print(my_range(1, 10)) # Output: [1, 2, 3, 4, 5, 6, 7, 8, 9]
    print(my_range(1, 10, 2)) # Output: [1, 3, 5, 7, 9]
    print(my_range(10, 1, -1)) # Output: [10, 9, 8, 7, 6, 5, 4, 3, 2]
