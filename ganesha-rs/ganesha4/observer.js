class Subject {
  constructor() {
    this.observers = [];
  }

  subscribe(observer) {
    this.observers.push(observer);
  }

  unsubscribe(observer) {
    this.observers = this.observers.filter(obs => obs !== observer);
  }

  notify(data) {
    this.observers.forEach(observer => observer.update(data));
  }
}

class Observer {
  constructor(name, updateFunction) {
    this.name = name;
    this.update = updateFunction;
  }
}

// Example Usage:
const subject = new Subject();

const observer1 = new Observer("Observer 1", (data) => {
  console.log(`Observer 1 received: ${data}`);
});

const observer2 = new Observer("Observer 2", (data) => {
  console.log(`Observer 2 received: ${data}`);
});

subject.subscribe(observer1);
subject.subscribe(observer2);

subject.notify("Hello from Subject!");

subject.unsubscribe(observer2);

subject.notify("Another message!");
