#include <stdio.h>

int factorial(int x) {
    if (x <= 1) return 1;
    return x * factorial(x - 1);
}

int main() {
    int r = factorial(5);
    printf("hello world");

    return 0;
}