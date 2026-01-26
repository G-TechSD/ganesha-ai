function pipe(...fns) {
  return function(x) {
    return fns.reduce((v, f) => f(v), x);
  }
}

// Example usage:
function add1(x) { return x + 1; }
function multiplyBy2(x) { return x * 2; }
function subtract3(x) { return x - 3; }

const myPipe = pipe(add1, multiplyBy2, subtract3);
console.log(myPipe(5)); // Output: (5 + 1) * 2 - 3 = 9
