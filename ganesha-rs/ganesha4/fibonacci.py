def fibonacci_sequence(n):
    """
    Generates a Fibonacci sequence up to n terms.
    """
    if n <= 0:
        return []
    elif n == 1:
        return [0]
    else:
        list_fib = [0, 1]
        while len(list_fib) < n:
            next_fib = list_fib[-1] + list_fib[-2]
            list_fib.append(next_fib)
        return list_fib

if __name__ == "__main__":
    num_terms = int(input("Enter the number of terms: "))
    fibonacci_numbers = fibonacci_sequence(num_terms)
    print("Fibonacci Sequence:", fibonacci_numbers)
