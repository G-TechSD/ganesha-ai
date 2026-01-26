import threading
import time
from collections import defaultdict

class PubSub:
    def __init__(self):
        self.subscribers = defaultdict(list)
        self.lock = threading.Lock()

    def subscribe(self, topic, callback):
        with self.lock:
            self.subscribers[topic].append(callback)

    def unsubscribe(self, topic, callback):
        with self.lock:
            try:
                self.subscribers[topic].remove(callback)
            except ValueError:
                pass  # Callback not found for this topic

    def publish(self, topic, message):
        with self.lock:
            for callback in self.subscribers[topic]:
                # Use a thread to avoid blocking the publisher
                threading.Thread(target=callback, args=(message,)).start()


if __name__ == '__main__':
    pubsub = PubSub()

    def callback1(message):
        print(f"Callback 1 received: {message}")
        time.sleep(1)  # Simulate some work

    def callback2(message):
        print(f"Callback 2 received: {message}")

    pubsub.subscribe("news", callback1)
    pubsub.subscribe("news", callback2)

    pubsub.publish("news", "Breaking news: Python pub/sub is awesome!")
    pubsub.publish("news", "Another news item!")

    pubsub.unsubscribe("news", callback2)
    pubsub.publish("news", "This is only for callback1")

    # Wait for threads to complete (optional, for demonstration)
    time.sleep(3)
    print("Done.")
