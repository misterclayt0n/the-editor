#include <stdio.h>

int factorial(int x) {
	if (x <= 1) return 1;
	return x * factorial(x - 1);
}

int main() {
	printf("hello world");
	return 0;
}