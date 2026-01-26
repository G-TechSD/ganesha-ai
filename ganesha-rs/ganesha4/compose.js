function compose(...funcs) {
  if (funcs.length === 0) {
    return (arg) => arg;
  }

  if (funcs.length === 1) {
    return funcs[0];
  }

  return funcs.reduce((a, b) => (...args) => a(b(...args)));
}

// Example usage:
function add(x) {
  return x + 2;
}

function multiply(x) {
  return x * 3;
}

function subtract(x) {
  return x - 1;
}

const composedFunction = compose(subtract, multiply, add); // subtract(multiply(add(x)))

console.log(composedFunction(5)); // Output: 20  ( (5 + 2) * 3 - 1 = 20 )

