import requests
from bs4 import BeautifulSoup

def scrape_titles(url):
    """
    Scrapes the titles from a webpage.

    Args:
        url (str): The URL of the webpage to scrape.

    Returns:
        list: A list of titles found on the webpage.
    """
    try:
        response = requests.get(url)
        response.raise_for_status()  # Raise HTTPError for bad responses (4xx or 5xx)
    except requests.exceptions.RequestException as e:
        print(f"Error fetching URL: {e}")
        return []

    soup = BeautifulSoup(response.content, 'html.parser')
    titles = [title.text.strip() for title in soup.find_all('title')]
    return titles

if __name__ == '__main__':
    url = input("Enter the URL of the webpage to scrape: ")
    titles = scrape_titles(url)

    if titles:
        print("Titles found on the webpage:")
        for title in titles:
            print(f"- {title}")
    else:
        print("No titles found or an error occurred.")
