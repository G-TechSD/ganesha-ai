class Heap {
  constructor(comparator = (a, b) => a - b) {
    this.heap = [];
    this.comparator = comparator;
  }

  size() {
    return this.heap.length;
  }

  isEmpty() {
    return this.size() === 0;
  }

  peek() {
    return this.heap[0];
  }

  push(value) {
    this.heap.push(value);
    this.heapifyUp();
  }

  pop() {
    if (this.isEmpty()) {
      return null;
    }
    if (this.size() === 1) {
      return this.heap.pop();
    }
    const root = this.peek();
    this.heap[0] = this.heap.pop();
    this.heapifyDown();
    return root;
  }

  heapifyUp() {
    let currentIndex = this.size() - 1;
    while (currentIndex > 0) {
      const parentIndex = Math.floor((currentIndex - 1) / 2);
      if (this.comparator(this.heap[currentIndex], this.heap[parentIndex]) < 0) {
        this.swap(currentIndex, parentIndex);
        currentIndex = parentIndex;
      } else {
        break;
      }
    }
  }

  heapifyDown() {
    let currentIndex = 0;
    while (true) {
      let leftChildIndex = 2 * currentIndex + 1;
      let rightChildIndex = 2 * currentIndex + 2;
      let smallestIndex = currentIndex;

      if (leftChildIndex < this.size() && this.comparator(this.heap[leftChildIndex], this.heap[smallestIndex]) < 0) {
        smallestIndex = leftChildIndex;
      }

      if (rightChildIndex < this.size() && this.comparator(this.heap[rightChildIndex], this.heap[smallestIndex]) < 0) {
        smallestIndex = rightChildIndex;
      }

      if (smallestIndex !== currentIndex) {
        this.swap(currentIndex, smallestIndex);
        currentIndex = smallestIndex;
      } else {
        break;
      }
    }
  }


  swap(i, j) {
    [this.heap[i], this.heap[j]] = [this.heap[j], this.heap[i]];
  }
}

// Example usage:
// const minHeap = new Heap(); // Min-heap
// minHeap.push(3);
// minHeap.push(1);
// minHeap.push(4);
// minHeap.push(1);
// minHeap.push(5);
// minHeap.push(9);
// minHeap.push(2);
// console.log(minHeap.pop()); // Output: 1
// console.log(minHeap.pop()); // Output: 1

// const maxHeap = new Heap((a, b) => b - a); // Max-heap
// maxHeap.push(3);
// maxHeap.push(1);
// maxHeap.push(4);
// maxHeap.push(1);
// maxHeap.push(5);
// maxHeap.push(9);
// maxHeap.push(2);
// console.log(maxHeap.pop()); // Output: 9
// console.log(maxHeap.pop()); // Output: 5
