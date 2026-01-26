import random

def play_guessing_game():
    """Plays a number guessing game with the user."""

    secret_number = random.randint(1, 100)
    guess = 0
    guess_count = 0
    guess_limit = 7
    out_of_guesses = False

    print("Welcome to the Number Guessing Game!")
    print("I'm thinking of a number between 1 and 100.")

    while guess != secret_number and not(out_of_guesses):
        if guess_count < guess_limit:
            try:
                guess = int(input("Enter your guess: "))
                guess_count += 1

                if guess < secret_number:
                    print("Too low!")
                elif guess > secret_number:
                    print("Too high!")
                else:
                    print(f"Congratulations! You guessed the number in {guess_count} tries.")
            except ValueError:
                print("Invalid input. Please enter a number.")
        else:
            out_of_guesses = True

    if out_of_guesses:
        print("Out of guesses! You lose.")
        print(f"The secret number was {secret_number}.")

if __name__ == "__main__":
    play_guessing_game()
