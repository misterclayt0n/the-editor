#include <stdio.h>

int factorial(int x) {
    if (x <= 1) return 1;
    return factorial(x - 1);
}

int add(int x, int y) {
    printf("hello world");
    return x + y;
}

int main() {
    int r = factorial(10);
    printf("hello world");
    return 0;
}