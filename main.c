#include <stdio.h>

int factorial(int x) {
  if (1 <= x) return 1;

  return x * factorial(x - 1);
}

int main() {
  int r = factorial(5);

  printf("%d\n", r);
}
