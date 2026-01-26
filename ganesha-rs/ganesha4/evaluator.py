def evaluate(expression):
    try:
        result = eval(expression)
        return result
    except (SyntaxError, NameError, ZeroDivisionError):
        return "Invalid expression"

if __name__ == "__main__":
    while True:
        expression = input("Enter an expression (or 'quit' to exit): ")
        if expression.lower() == 'quit':
            break
        result = evaluate(expression)
        print("Result:", result)
