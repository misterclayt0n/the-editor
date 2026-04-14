#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define MIN_VALUE 1
#define MAX_VALUE 100

static int read_int(const char *prompt, int *out) {
	printf("%s", prompt);
	if (scanf("%d", out) != 1) {
		return -1;
	}

	return 0;
}

int main(void) {
    srand((unsigned int)time(NULL));

    int secret = MIN_VALUE + rand() % (MAX_VALUE - MIN_VALUE + 1);
    int guesses = 0;
    int guess;

    printf("I'm thinking of a number between %d and %d.\n", MIN_VALUE, MAX_VALUE);
    printf("Try to guess it.\n");

    for (;;) {
        if (read_int("Your guess: ", &guess) != 0) {
            printf("That is not a valid number. Goodbye.\n");
            return 1;
        }

        guesses++;

        if (guess < secret) {
            printf("Too low.\n");
        } else if (guess > secret) {
            printf("Too high.\n");
        } else {
            printf("You got it in %d guess%s. The number was %d.\n",
                   guesses,
                   guesses == 1 ? "" : "es",
                   secret);
            return 0;
        }
    }
}
