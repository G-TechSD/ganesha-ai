class StateMachine:
    def __init__(self, initial_state):
        self.current_state = initial_state
        self.states = {}

    def add_state(self, state_name, state_object):
        self.states[state_name] = state_object

    def run(self, input):
        if input in self.states[self.current_state].transitions:
            next_state = self.states[self.current_state].transitions[input]
            self.current_state = next_state
            print(f"Transitioned to state: {self.current_state}")
        else:
            print(f"No transition defined for input '{input}' in state {self.current_state}")

    def get_current_state(self):
        return self.current_state


class State:
    def __init__(self):
        self.transitions = {}

    def add_transition(self, input, next_state):
        self.transitions[input] = next_state


if __name__ == '__main__':
    # Example usage:
    machine = StateMachine("state1")

    state1 = State()
    state2 = State()
    state3 = State()

    state1.add_transition("A", "state2")
    state1.add_transition("B", "state3")
    state2.add_transition("C", "state1")
    state3.add_transition("D", "state2")

    machine.add_state("state1", state1)
    machine.add_state("state2", state2)
    machine.add_state("state3", state3)

    print(f"Current state: {machine.get_current_state()}")
    machine.run("A")
    print(f"Current state: {machine.get_current_state()}")
    machine.run("C")
    print(f"Current state: {machine.get_current_state()}")
    machine.run("B")
    print(f"Current state: {machine.get_current_state()}")
    machine.run("D")
    print(f"Current state: {machine.get_current_state()}")
    machine.run("E") # No transition defined
    print(f"Current state: {machine.get_current_state()}")
