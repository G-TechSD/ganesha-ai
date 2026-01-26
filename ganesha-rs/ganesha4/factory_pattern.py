class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        raise NotImplementedError("Subclasses must implement speak method")


class Dog(Animal):
    def speak(self):
        return "Woof!"


class Cat(Animal):
    def speak(self):
        return "Meow!"


class AnimalFactory:
    def create_animal(self, animal_type, name):
        if animal_type == "dog":
            return Dog(name)
        elif animal_type == "cat":
            return Cat(name)
        else:
            raise ValueError("Invalid animal type")


# Example usage
factory = AnimalFactory()

dog = factory.create_animal("dog", "Buddy")
print(f"{dog.name} says: {dog.speak()}")

cat = factory.create_animal("cat", "Whiskers")
print(f"{cat.name} says: {cat.speak()}")

