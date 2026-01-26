import functools

class LRUCache:
    def __init__(self, capacity: int):
        self.capacity = capacity
        self.cache = {}
        self.usage = {}
        self.order = 0

    def get(self, key: int) -> int:
        if key in self.cache:
            self.usage[key] = self.order
            self.order += 1
            return self.cache[key]
        else:
            return -1

    def put(self, key: int, value: int) -> None:
        if key in self.cache:
            self.cache[key] = value
            self.usage[key] = self.order
            self.order += 1
        else:
            if len(self.cache) >= self.capacity:
                lru_key = min(self.usage, key=self.usage.get)
                del self.cache[lru_key]
                del self.usage[lru_key]
            self.cache[key] = value
            self.usage[key] = self.order
            self.order += 1
