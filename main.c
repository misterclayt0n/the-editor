#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define MIN 1
#define MAX 100

static unsigned long long factorial(unsigned n) {
    unsigned long long result = 1;
    for (unsigned i = 2; i <= n; i++) {
        result *= i;
    }
    return result;
}

int main(void) {
    srand((unsigned)time(NULL));
    int secret = MIN + rand() % (MAX - MIN + 1);
    int guess;
    int tries = 0;

    printf("I'm thinking of a number between %d and %d.\n", MIN, MAX);
    printf("Can you guess it?\n");

    for (;;) {
        printf("Your guess: ");
        if (scanf("%d", &guess) != 1) {
            printf("That's not a number. Goodbye.\n");
            return 1;
        }
        tries++;

        if (guess < secret) {
            printf("Too low.\n");
        } else if (guess > secret) {
            printf("Too high.\n");
        } else {
            printf("You got it in %d %s!\n", tries, tries == 1 ? "try" : "tries");
            if (tries <= 20) {
                printf("%d! = %llu\n", tries, factorial((unsigned)tries));
            }
            return 0;
        }
    }
}
