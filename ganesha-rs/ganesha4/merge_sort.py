def merge_sort(arr):
    """
    Sorts an array using the Merge Sort algorithm.

    Parameters:
        arr (list): The list of comparable elements to sort.

    Returns:
        list: A new sorted list.
    """
    if len(arr) <= 1:
        return arr

    mid = len(arr) // 2
    left_half = merge_sort(arr[:mid])
    right_half = merge_sort(arr[mid:])

    return _merge(left_half, right_half)

def _merge(left, right):
    """Helper function to merge two sorted lists."""
    merged = []
    i = j = 0

    # Merge the two halves
    while i < len(left) and j < len(right):
        if left[i] <= right[j]:
            merged.append(left[i])
            i += 1
        else:
            merged.append(right[j])
            j += 1

    # Append any remaining elements
    merged.extend(left[i:])
    merged.extend(right[j:])

    return merged

if __name__ == "__main__":
    import random
    sample = [random.randint(0, 100) for _ in range(20)]
    print("Unsorted:", sample)
    sorted_list = merge_sort(sample)
    print("Sorted:  ", sorted_list)
