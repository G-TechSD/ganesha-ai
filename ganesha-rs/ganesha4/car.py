class Car:
    def __init__(self, make, model):
        self.make = make
        self.model = model

    def __str__(self):
        return f"{self.make} {self.model}"

if __name__ == '__main__':
    my_car = Car("Toyota", "Camry")
    print(my_car)
