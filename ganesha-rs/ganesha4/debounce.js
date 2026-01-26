function debounce(func, delay) {
  let timeoutId;
  return function(...args) {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => {
      func.apply(this, args);
    }, delay);
  };
}

// Example usage:
function myExpensiveFunction(arg) {
  console.log('Executing expensive function with argument:', arg);
}

const debouncedFunction = debounce(myExpensiveFunction, 300);

// Call the debounced function multiple times
debouncedFunction('First call');
debouncedFunction('Second call');
debouncedFunction('Third call');

// Only the last call within the 300ms delay will be executed
