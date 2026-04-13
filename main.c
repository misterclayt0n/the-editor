#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define MIN_VALUE 1
#define MAX_VALUE 100

static int read_guess(void) {
    char line[64];
    long n;

    if (fgets(line, sizeof line, stdin) == NULL) {
        return -1;
    }
    n = strtol(line, NULL, 10);
    if (n < MIN_VALUE || n > MAX_VALUE) {
        return -2;
    }
    return (int)n;
}

int main(void) {
    int secret, guess, tries = 0;

    srand((unsigned)time(NULL));
    secret = MIN_VALUE + rand() % (MAX_VALUE - MIN_VALUE + 1);

    printf("I'm thinking of a number from %d to %d. Can you guess it?\n",
           MIN_VALUE, MAX_VALUE);

    for (;;) {
        printf("Your guess: ");
        fflush(stdout);

        guess = read_guess();
        if (guess == -1) {
            printf("\nGoodbye.\n");
            return 0;
        }
        if (guess == -2) {
            printf("Please enter a whole number between %d and %d.\n",
                   MIN_VALUE, MAX_VALUE);
            continue;
        }

        tries++;
        if (guess < secret) {
            printf("Too low.\n");
        } else if (guess > secret) {
            printf("Too high.\n");
        } else {
            printf("You got it in %d %s. The number was %d.\n", tries,
                   tries == 1 ? "try" : "tries", secret);
            return 0;
        }
    }
}
