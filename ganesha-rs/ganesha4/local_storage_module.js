const localStorageModule = {
  get: function(key) {
    try {
      const serializedValue = localStorage.getItem(key);
      if (serializedValue === null) {
        return undefined;
      }
      return JSON.parse(serializedValue);
    } catch (e) {
      console.error('Error getting data from localStorage:', e);
      return undefined;
    }
  },

  set: function(key, value) {
    try {
      const serializedValue = JSON.stringify(value);
      localStorage.setItem(key, serializedValue);
    } catch (e) {
      console.error('Error setting data to localStorage:', e);
    }
  },

  clear: function(key) {
    try {
      localStorage.removeItem(key);
    } catch (e) {
      console.error('Error clearing data from localStorage:', e);
    }
  },

  clearAll: function() {
    try {
      localStorage.clear();
    } catch (e) {
      console.error('Error clearing all data from localStorage:', e);
    }
  }
};

export default localStorageModule;
