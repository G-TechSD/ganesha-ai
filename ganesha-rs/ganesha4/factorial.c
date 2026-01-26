#include <stdio.h>
#include <stdlib.h>

/* Computes the factorial of n (n >= 0).
   Returns 1 for n == 0.
*/
unsigned long long factorial(unsigned int n) {
    unsigned long long result = 1;
    for (unsigned int i = 2; i <= n; ++i) {
        result *= i;
    }
    return result;
}

int main(void) {
    unsigned int number;

    printf("Enter a non‑negative integer: ");
    if (scanf("%u", &number) != 1) {
        fprintf(stderr, "Invalid input.\n");
        return EXIT_FAILURE;
    }

    unsigned long long fact = factorial(number);
    printf("Factorial of %u is %llu\\n", number, fact);

    return EXIT_SUCCESS;
}
