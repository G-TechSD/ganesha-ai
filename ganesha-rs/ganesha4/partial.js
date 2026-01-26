function partial(func, ...args) {
  return function(...newArgs) {
    return func(...args, ...newArgs);
  };
}

// Example usage:
function greet(greeting, name) {
  return `${greeting}, ${name}!`;
}

const sayHello = partial(greet, "Hello");
console.log(sayHello("World")); // Output: Hello, World!

const greetJohn = partial(greet, "Hello", "John");
console.log(greetJohn()); // Output: Hello, John!
