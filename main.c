#include <stdio.h>

int factorial(int x) {
	if (x <= 1) return 1;
	return x * factorial(x - 1);
}

int add(int x, int y) {
	return x + y;
}

int main() {
	int r = factorial(5);
	int r2 = add(1, 2);
	printf("hi fellas");
	
	return 0;
}