"""
Simple Todo List Module

Provides two functions:
- add_task(todo_list, task): Appends a new task to the list.
- remove_task(todo_list, index): Removes the task at the given 0‑based index.

Example usage:

>>> tasks = []
>>> add_task(tasks, "Buy milk")
>>> add_task(tasks, "Write report")
>>> tasks
['Buy milk', 'Write report']
>>> remove_task(tasks, 0)
>>> tasks
['Write report']
"""

def add_task(todo_list, task):
    """
    Add a new task to the todo list.

    Parameters:
        todo_list (list): The list of current tasks.
        task (str): The task description to add.

    Returns:
        None
    """
    if not isinstance(task, str) or not task.strip():
        raise ValueError("Task must be a non-empty string.")
    todo_list.append(task)

def remove_task(todo_list, index):
    """
    Remove the task at the specified index from the todo list.

    Parameters:
        todo_list (list): The list of current tasks.
        index (int): Zero-based position of the task to remove.

    Returns:
        None

    Raises:
        IndexError: If the index is out of range.
    """
    try:
        del todo_list[index]
    except IndexError as e:
        raise IndexError(f"Index {index} out of range for todo list.") from e

# Simple command‑line interface for quick testing
if __name__ == "__main__":
    import sys

    tasks = []

    while True:
        cmd = input("\nEnter command (add/remove/list/quit): ").strip().lower()
        if cmd == "add":
            task_desc = input("Task: ")
            try:
                add_task(tasks, task_desc)
                print(f"Added: {task_desc}")
            except ValueError as ve:
                print(ve)
        elif cmd == "remove":
            idx_str = input("Index to remove (0-based): ").strip()
            if not idx_str.isdigit():
                print("Please enter a valid integer index.")
                continue
            idx = int(idx_str)
            try:
                removed = tasks[idx]
                remove_task(tasks, idx)
                print(f"Removed: {removed}")
            except IndexError as ie:
                print(ie)
        elif cmd == "list":
            if not tasks:
                print("Todo list is empty.")
            else:
                for i, t in enumerate(tasks):
                    print(f"{i}: {t}")
        elif cmd == "quit":
            print("Goodbye!")
            sys.exit(0)
        else:
            print("Unknown command. Available: add, remove, list, quit.")
