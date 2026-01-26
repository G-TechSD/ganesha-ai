class EventEmitter {
  constructor() {
    this.events = {};
  }

  subscribe(event, callback) {
    if (!this.events[event]) {
      this.events[event] = [];
    }
    this.events[event].push(callback);

    return {
      unsubscribe: () => {
        this.events[event] = this.events[event].filter(cb => cb !== callback);
        if (this.events[event].length === 0) {
          delete this.events[event];
        }
      }
    };
  }

  emit(event, ...args) {
    if (this.events[event]) {
      this.events[event].forEach(callback => {
        callback(...args);
      });
    }
  }
}

