class Rectangle:
    def __init__(self, width: float, height: float):
        self.width = width
        self.height = height

    def area(self) -> float:
        return self.width * self.height

# Example usage:
if __name__ == "__main__":
    rect = Rectangle(3, 4)
    print(f"Area: {rect.area()}")
