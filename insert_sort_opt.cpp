#include <iostream>
#include <cstring>   // for memmove

void insertionSort(int *a, int n) {
    for (int i = 1; i < n; ++i) {
        int key = a[i];
        int lo = 0, hi = i;
        while (lo < hi) {          // binary search
            int mid = (lo + hi) / 2;
            if (a[mid] <= key) lo = mid + 1;
            else                hi = mid;
        }
        int pos = lo;              // insertion point
        memmove(&a[pos+1], &a[pos], (i - pos)*sizeof(int));
        a[pos] = key;
    }
}

int main() {
    int arr[] = {5, 2, 9, 1, 5, 6};
    int n = sizeof(arr)/sizeof(arr[0]);

    insertionSort(arr, n);

    // simple pointer loop to print the sorted array
    for (int *p = arr; p < arr + n; ++p)
        std::cout << *p << ' ';
    std::cout << '\n';
}
