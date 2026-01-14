#include <stdio.h>

int add(int x, int y) {
	return x + y;
}


// yes 
int factorial(int x) {
	if (x <= 1) return 1;

	return x * factorial(x - 1);
}

int main() {
	int r = add(1, 2);
	
	printf("%d\n", r);
	
	return 0;
}
