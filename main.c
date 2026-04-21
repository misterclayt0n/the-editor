#include <stdio.h>
#include <stdlib.h>
#include <time.h>

typedef struct {
	int    *items;
	size_t count;
	size_t capacity;
} Numbers;

int main() {
	Numbers xs = {0};
	
	for (int x = 0; x < 10; x++) {
		if (xs.capacity == 0) {
			xs.capacity = 4;
			xs.items = malloc(xs.capacity * sizeof(int));
		}
		
		if (xs.count >= xs.capacity) {
			xs.capacity = xs.capacity ? xs.capacity * 2 : 4;
			xs.items = realloc(xs.items, xs.capacity * sizeof(int));
		}
		
		xs.items[xs.count++] = x;
	}
	return 0;
}

