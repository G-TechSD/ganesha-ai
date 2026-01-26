class SimpleSet {
  constructor() {
    this.data = {};
    this.size = 0;
  }

  add(value) {
    if (!this.has(value)) {
      this.data[value] = true;
      this.size++;
    }
  }

  remove(value) {
    if (this.has(value)) {
      delete this.data[value];
      this.size--;
    }
  }

  has(value) {
    return this.data.hasOwnProperty(value);
  }

  values() {
    return Object.keys(this.data);
  }

  getSize() {
    return this.size;
  }

  clear() {
    this.data = {};
    this.size = 0;
  }
}

// Example usage:
const mySet = new SimpleSet();
mySet.add(1);
mySet.add(2);
mySet.add(3);

console.log("Set contains 1:", mySet.has(1)); // true
console.log("Set contains 4:", mySet.has(4)); // false
console.log("Set values:", mySet.values()); // ["1", "2", "3"]
console.log("Set size:", mySet.getSize()); // 3

mySet.remove(2);
console.log("Set values after removing 2:", mySet.values()); // ["1", "3"]
console.log("Set size after removing 2:", mySet.getSize()); // 2

mySet.clear();
console.log("Set size after clearing:", mySet.getSize()); // 0
