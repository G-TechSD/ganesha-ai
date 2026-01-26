import json

def extract_keys(json_data, keys=None):
    """
    Recursively extracts all keys from a JSON object.

    Args:
        json_data: The JSON object (dict or list) to parse.
        keys: A set to store the extracted keys (used in recursive calls).

    Returns:
        A set containing all unique keys found in the JSON object.
    """
    if keys is None:
        keys = set()

    if isinstance(json_data, dict):
        for key, value in json_data.items():
            keys.add(key)
            extract_keys(value, keys)  # Recursive call for nested objects/lists
    elif isinstance(json_data, list):
        for item in json_data:
            extract_keys(item, keys)  # Recursive call for list items

    return keys

def main():
    # Example usage:
    json_string = '{"name": "John Doe", "age": 30, "address": {"street": "123 Main St", "city": "Anytown"}, "hobbies": ["reading", "hiking"]}'
    try:
        data = json.loads(json_string)
        all_keys = extract_keys(data)
        print("Extracted Keys:", all_keys)
    except json.JSONDecodeError as e:
        print(f"Error decoding JSON: {e}")

if __name__ == "__main__":
    main()
