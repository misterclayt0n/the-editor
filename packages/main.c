#include <stdio.h>

int factorial(int x){
  if (x <= 1) return 1;
  return x * factorial(x - 1);
}

int main() {
  // this is a test comment actuallyy

  // hi how ya doin?
  int r = factorial(10);
  printf("hello world");
  return 0;
}
