#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define MIN 1
#define MAX 100

static int read_int(const char *prompt, int *out) {
    char line[64];

    for (;;) {
        fputs(prompt, stdout);
        fflush(stdout);
        if (!fgets(line, sizeof line, stdin)) {
            return 0;
        }
        char *end = NULL;
        long v = strtol(line, &end, 10);
        if (end != line && *end != '\0' && *end != '\n') {
            puts("Please enter a whole number.");
            continue;
        }
        if (end == line) {
            puts("Please enter a whole number.");
            continue;
        }
        if (v < MIN || v > MAX) {
            printf("Pick a number between %d and %d.\n", MIN, MAX);
            continue;
        }
        *out = (int)v;
        return 1;
    }
}

int main(void) {
    srand((unsigned)time(NULL));
    int secret = MIN + rand() % (MAX - MIN + 1);
    int guess = 0;
    int tries = 0;

    printf("I'm thinking of a number from %d to %d. Can you guess it?\n", MIN, MAX);

    while (1) {
        if (!read_int("Your guess: ", &guess)) {
            puts("\nGoodbye.");
            return 0;
        }
        tries++;

        if (guess < secret) {
            puts("Too low.");
        } else if (guess > secret) {
            puts("Too high.");
        } else {
            printf("You got it in %d %s. The number was %d.\n",
                   tries,
                   tries == 1 ? "try" : "tries",
                   secret);
            return 0;
        }
    }
}
