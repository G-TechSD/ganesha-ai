#!/usr/bin/env python3
"""
Simple Tic-Tac-Toe game for two players.
"""

def print_board(board):
    """Print the current state of the board."""
    print("\n")
    for i in range(3):
        row = " | ".join(board[i])
        print(f" {row} ")
        if i < 2:
            print("---+---+---")
    print("\n")

def check_winner(board, mark):
    """Return True if the given mark has won."""
    # Rows, columns and diagonals
    win_states = [
        [board[0][0], board[0][1], board[0][2]],
        [board[1][0], board[1][1], board[1][2]],
        [board[2][0], board[2][1], board[2][2]],
        [board[0][0], board[1][0], board[2][0]],
        [board[0][1], board[1][1], board[2][1]],
        [board[0][2], board[1][2], board[2][2]],
        [board[0][0], board[1][1], board[2][2]],
        [board[0][2], board[1][1], board[2][0]]
    ]
    return any(all(cell == mark for cell in line) for line in win_states)

def board_full(board):
    """Return True if the board is full."""
    return all(cell != ' ' for row in board for cell in row)

def get_move(player, board):
    """Prompt player to enter a move and validate it."""
    while True:
        try:
            move = input(f"Player {player} ({'X' if player == 1 else 'O'}), enter your move as row,col (0-2): ")
            row, col = map(int, move.split(','))
            if not (0 <= row <= 2 and 0 <= col <= 2):
                print("Coordinates must be between 0 and 2.")
                continue
            if board[row][col] != ' ':
                print("That cell is already taken. Choose another one.")
                continue
            return row, col
        except ValueError:
            print("Invalid format. Please enter as row,col (e.g., 1,2).")

def main():
    board = [[' ' for _ in range(3)] for _ in range(3)]
    current_player = 1  # Player 1 uses X, Player 2 uses O
    marks = {1: 'X', 2: 'O'}

    print("Welcome to Tic-Tac-Toe!")
    print_board(board)

    while True:
        row, col = get_move(current_player, board)
        board[row][col] = marks[current_player]
        print_board(board)

        if check_winner(board, marks[current_player]):
            print(f"Player {current_player} ({marks[current_player]}) wins!")
            break
        if board_full(board):
            print("The game is a draw.")
            break

        # Switch player
        current_player = 2 if current_player == 1 else 1

if __name__ == "__main__":
    main()
