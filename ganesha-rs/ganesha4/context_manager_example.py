class MyContextManager:
    def __init__(self, resource_name):
        self.resource_name = resource_name
        self.resource = None

    def __enter__(self):
        print(f"Entering the context. Acquiring resource: {self.resource_name}")
        # Simulate resource acquisition (e.g., opening a file)
        self.resource = open(self.resource_name, 'w')  # Open for writing
        return self.resource

    def __exit__(self, exc_type, exc_val, exc_tb):
        print(f"Exiting the context. Releasing resource: {self.resource_name}")
        if self.resource:
            self.resource.close()
        if exc_type:
            print(f"Exception occurred: {exc_type}, {exc_val}")
            # Handle the exception (e.g., log it, re-raise it)
            return False  # Re-raise the exception
        return True  # Suppress the exception

# Example usage:
if __name__ == "__main__":
    with MyContextManager("example.txt") as f:
        f.write("Hello, context manager!")
        # Simulate an exception
        # raise ValueError("Something went wrong!")
    print("Context manager finished.")

    # Verify that the file was created and written to
    try:
        with open("example.txt", "r") as f:
            content = f.read()
            print(f"Content of example.txt: {content}")
    except FileNotFoundError:
        print("example.txt not found (likely an exception was raised and not handled)")

