/**
 * Simple Calculator
 *
 * Provides basic arithmetic operations:
 * - add(a, b)
 * - subtract(a, b)
 * - multiply(a, b)
 * - divide(a, b)  (throws on division by zero)
 */

export function add(a, b) {
    return a + b;
}

export function subtract(a, b) {
    return a - b;
}

export function multiply(a, b) {
    return a * b;
}

export function divide(a, b) {
    if (b === 0) {
        throw new Error('Division by zero');
    }
    return a / b;
}
