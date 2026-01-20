def hello(name):
    """Say hello to someone."""
    print(f"Hello, {name}!")

def add(a, b):
    # Add two numbers
    return a + b

if __name__ == "__main__":
    hello("World")
    result = add(2, 3)
    print(f"2 + 3 = {result}")
