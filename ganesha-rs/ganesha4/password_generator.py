import random
import string

def generate_password(length=12):
  """Generates a random password of the specified length.

  Args:
    length: The length of the password to generate.

  Returns:
    A random password string.
  """

  characters = string.ascii_letters + string.digits + string.punctuation
  password = ''.join(random.choice(characters) for i in range(length))
  return password

if __name__ == '__main__':
  password = generate_password()
  print(f"Generated password: {password}")
