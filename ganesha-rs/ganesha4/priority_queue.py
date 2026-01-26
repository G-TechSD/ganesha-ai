import heapq

class PriorityQueue:
    def __init__(self):
        self._queue = []
        self._index = 0

    def push(self, item, priority):
        heapq.heappush(self._queue, (-priority, self._index, item))
        self._index += 1

    def pop(self):
        return heapq.heappop(self._queue)[-1]

    def is_empty(self):
        return len(self._queue) == 0

# Example usage:
if __name__ == '__main__':
    pq = PriorityQueue()
    pq.push("Task 1", 3)
    pq.push("Task 2", 1)
    pq.push("Task 3", 2)

    while not pq.is_empty():
        print(pq.pop())
