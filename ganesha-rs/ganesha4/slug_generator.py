import re

def slugify(text):
    text = text.lower()
    text = re.sub(r'[^a-z0-9-]+', '-', text)
    text = re.sub(r'-+', '-', text)
    text = text.strip('-')
    return text

if __name__ == '__main__':
    text = input("Enter text to slugify: ")
    slug = slugify(text)
    print(f"Slug: {slug}")
