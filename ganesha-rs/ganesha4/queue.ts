class Queue<T> {
  private items: T[] = [];

  enqueue(item: T): void {
    this.items.push(item);
  }

  dequeue(): T | undefined {
    return this.items.shift();
  }

  peek(): T | undefined {
    if (!this.isEmpty()) {
      return this.items[0];
    }
    return undefined;
  }

  isEmpty(): boolean {
    return this.items.length === 0;
  }

  size(): number {
    return this.items.length;
  }

  printQueue(): void {
    console.log(this.items);
  }
}

// Example usage:
const queue = new Queue<number>();

queue.enqueue(1);
queue.enqueue(2);
queue.enqueue(3);

queue.printQueue(); // Output: [1, 2, 3]

console.log("Dequeue:", queue.dequeue()); // Output: Dequeue: 1
console.log("Peek:", queue.peek()); // Output: Peek: 2
console.log("Is empty:", queue.isEmpty()); // Output: Is empty: false
console.log("Size:", queue.size()); // Output: Size: 2

queue.printQueue(); // Output: [2, 3]
