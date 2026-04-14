#include <stdio.h>

int factorial(int n) {
    if (n == 0) {
        return 1;
    }
    return n * factorial(n - 1);
}

int main() {
    int r = factorial(5);
    printf("Hello, World!\n");
    return 0;
}
