import sys

def word_count(filename):
    """Counts the number of words in a file."""
    try:
        with open(filename, 'r') as f:
            text = f.read()
    except FileNotFoundError:
        print(f"Error: File '{filename}' not found.")
        return None

    words = text.split()
    return len(words)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python word_counter.py <filename>")
    else:
        filename = sys.argv[1]
        count = word_count(filename)
        if count is not None:
            print(f"The file '{filename}' contains {count} words.")
