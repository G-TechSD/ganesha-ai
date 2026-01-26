function promisify(fn) {
  return function (...args) {
    return new Promise((resolve, reject) => {
      fn.call(this, ...args, (err, result) => {
        if (err) {
          reject(err);
        } else {
          resolve(result);
        }
      });
    });
  };
}

// Example usage:
function myAsyncFunction(arg1, arg2, callback) {
  setTimeout(() => {
    if (arg1 > 0) {
      callback(null, arg1 + arg2);
    } else {
      callback(new Error("arg1 must be positive"));
    }
  }, 500);
}

const myPromiseFunction = promisify(myAsyncFunction);

myPromiseFunction(5, 10)
  .then(result => {
    console.log("Result:", result); // Output: Result: 15
  })
  .catch(err => {
    console.error("Error:", err);
  });

myPromiseFunction(-1, 10)
  .then(result => {
    console.log("Result:", result);
  })
  .catch(err => {
    console.error("Error:", err); // Output: Error: Error: arg1 must be positive
  });
