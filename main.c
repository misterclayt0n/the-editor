#include <stdio.h>

int add(int a, int b) {
  return a + b;
}

int main() {
  int r = add(1, 2);
  
  printf("%d\n", r);
  return 0;
}
