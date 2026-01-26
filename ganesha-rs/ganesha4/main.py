def show_location(location):
    print("\n" + "#" * 30)
    print(f"You are in the {location['name']}")
    print(location['description'])

    if location['items']:
        print("You see the following items:")
        for item in location['items']:
            print(f"- {item}")

    if location['exits']:
        print("You can go to:")
        for direction, destination in location['exits'].items():
            print(f"- {direction}")

def get_command():
    return input("\nWhat do you do? > ").lower().split()

def move(player, direction):
    location = player['location']
    if direction in location['exits']:
        new_location_name = location['exits'][direction]
        new_location = locations[new_location_name]
        player['location'] = new_location
        show_location(new_location)
    else:
        print("You can't go that way.")

def take_item(player, item_name):
    location = player['location']
    if item_name in location['items']:
        player['inventory'].append(item_name)
        location['items'].remove(item_name)
        print(f"You take the {item_name}.")
    else:
        print("That item isn't here.")

def inventory(player):
    if player['inventory']:
        print("You are carrying:")
        for item in player['inventory']:
            print(f"- {item}")
    else:
        print("You aren't carrying anything.")

# Game setup
locations = {
    "forest": {
        "name": "Forest",
        "description": "You are in a dense forest. Sunlight barely reaches the ground.",
        "items": ["sword", "potion"],
        "exits": {"north": "cave", "east": "clearing"}
    },
    "cave": {
        "name": "Cave",
        "description": "You are in a dark, damp cave. Water drips from the ceiling.",
        "items": ["torch"],
        "exits": {"south": "forest"}
    },
    "clearing": {
        "name": "Clearing",
        "description": "You are in a small clearing. There's a path leading further east.",
        "items": [],
        "exits": {"west": "forest", "east": "path"}
    },
    "path": {
        "name": "Path",
        "description": "You are on a winding path.  You can hear a stream nearby.",
        "items": ["apple"],
        "exits": {"west": "clearing", "north": "stream"}
    },
    "stream": {
        "name": "Stream",
        "description": "You are next to a clear stream. The water looks refreshing.",
        "items": [],
        "exits": {"south": "path"}
    }
}

player = {
    "name": "Player",
    "location": locations["forest"],
    "inventory": []
}

# Game loop
show_location(player['location'])

while True:
    command = get_command()

    if not command:
        print("Enter a command.")
        continue

    action = command[0]

    if action == "go":
        if len(command) > 1:
            direction = command[1]
            move(player, direction)
        else:
            print("Go where?")
    elif action == "take":
        if len(command) > 1:
            item_name = command[1]
            take_item(player, item_name)
        else:
            print("Take what?")
    elif action == "inventory":
        inventory(player)
    elif action == "quit":
        print("Thanks for playing!")
        break
    else:
        print("Invalid command.")
