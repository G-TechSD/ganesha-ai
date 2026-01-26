function throttle(func, delay) {
  let timeoutId;
  let lastExecTime = 0;

  return function(...args) {
    const currentTime = Date.now();
    const timeSinceLastExec = currentTime - lastExecTime;

    if (!timeoutId) {
      if (timeSinceLastExec >= delay) {
        // Execute immediately if enough time has passed since last execution
        func.apply(this, args);
        lastExecTime = currentTime;
      } else {
        // Schedule execution for the remaining time
        timeoutId = setTimeout(() => {
          func.apply(this, args);
          lastExecTime = Date.now();
          timeoutId = null;
        }, delay - timeSinceLastExec);
      }
    }
  };
}

// Example usage:
function myThrottledFunction() {
  console.log("Throttled function executed!");
}

const throttled = throttle(myThrottledFunction, 1000); // Throttle to once per 1000ms

// Simulate rapid calls:
throttled();
throttled();
throttled(); // Only the first call (or after 1000ms) will execute the function

