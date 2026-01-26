def is_palindrome(s):
  """
  Checks if a string is a palindrome (reads the same forwards and backward).

  Args:
    s: The string to check.

  Returns:
    True if the string is a palindrome, False otherwise.
  """
  s = s.lower()  # Ignore case
  s = ''.join(filter(str.isalnum, s))  # Remove non-alphanumeric characters
  return s == s[::-1]

if __name__ == '__main__':
  test_string1 = "Racecar"
  test_string2 = "A man, a plan, a canal: Panama"
  test_string3 = "hello"

  print(f'"{test_string1}" is a palindrome: {is_palindrome(test_string1)}')
  print(f'"{test_string2}" is a palindrome: {is_palindrome(test_string2)}')
  print(f'"{test_string3}" is a palindrome: {is_palindrome(test_string3)}')
