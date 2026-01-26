def binary_search(list, target):
  """
  Performs a binary search on a sorted list to find the index of a target value.

  Args:
    list: A sorted list of elements.
    target: The value to search for in the list.

  Returns:
    The index of the target value in the list if found, otherwise -1.
  """
  low = 0
  high = len(list) - 1

  while low <= high:
    mid = (low + high) // 2
    guess = list[mid]

    if guess == target:
      return mid
    if guess > target:
      high = mid - 1
    else:
      low = mid + 1

  return -1

# Example usage:
if __name__ == "__main__":
  my_list = [2, 5, 7, 8, 11, 12]
  target = 13
  index = binary_search(my_list, target)

  if index != -1:
    print(f"Target {target} found at index {index}")
  else:
    print(f"Target {target} not found in the list")
