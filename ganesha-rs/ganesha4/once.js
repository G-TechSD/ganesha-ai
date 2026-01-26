function once(fn) {
  let called = false;
  let result;

  return function(...args) {
    if (!called) {
      called = true;
      result = fn.apply(this, args);
      // Optionally, nullify the original function to free up memory
      // fn = null; 
      return result;
    }
    return result; // Or return undefined, depending on desired behavior
  };
}

// Example usage:
function myFunc(x) {
  console.log("Executing myFunc with:", x);
  return x * 2;
}

const onceFunc = once(myFunc);

console.log(onceFunc(5));  // Output: Executing myFunc with: 5, 10
console.log(onceFunc(10)); // Output: 10 (myFunc is not executed again)
console.log(onceFunc(15)); // Output: 10 (myFunc is not executed again)
