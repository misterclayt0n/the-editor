#include <stdio.h>

int facotriual(int x) {
	if (x <= 1) return 1;

	return x * facotriual(x - 1);
}

int main() {
	int r = facotriual(5);
	printf("hello world");
}
