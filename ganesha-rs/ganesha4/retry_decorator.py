import time
import functools

def retry(max_retries=3, delay=1):
    """
    A decorator to retry a function a specified number of times with a delay between retries.

    Args:
        max_retries (int): The maximum number of times to retry the function. Defaults to 3.
        delay (int): The delay in seconds between retries. Defaults to 1.
    """
    def decorator_retry(func):
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            retries = 0
            while retries < max_retries:
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    print(f"Attempt {retries + 1} failed: {e}")
                    retries += 1
                    time.sleep(delay)
            print(f"Function {func.__name__} failed after {max_retries} retries.")
            raise  # Re-raise the last exception
        return wrapper
    return decorator_retry

if __name__ == '__main__':
    @retry(max_retries=5, delay=2)
    def flaky_function():
        """This function sometimes fails."""
        import random
        if random.random() < 0.5:
            raise ValueError("Something went wrong!")
        else:
            return "Success!"

    try:
        result = flaky_function()
        print(f"Result: {result}")
    except Exception as e:
        print(f"Final error: {e}")
