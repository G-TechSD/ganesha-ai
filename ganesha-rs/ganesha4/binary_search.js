/**
 * Binary Search implementation for sorted arrays.
 *
 * @param {number[]} arr - Sorted array of numbers.
 * @param {number} target - Value to search for.
 * @returns {number} Index of the target if found, otherwise -1.
 */
function binarySearch(arr, target) {
  let left = 0;
  let right = arr.length - 1;

  while (left <= right) {
    const mid = Math.floor((left + right) / 2);
    const midVal = arr[mid];

    if (midVal === target) return mid;
    if (midVal < target) left = mid + 1;
    else right = mid - 1;
  }
  return -1;
}

// Demo usage
const sampleArray = [3, 7, 15, 22, 28, 35, 42, 56, 63];
const targets = [22, 5, 63, 100];

targets.forEach(t => {
  const idx = binarySearch(sampleArray, t);
  console.log(`Target ${t} -> Index: ${idx}`);
});
