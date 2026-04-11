#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define MIN 1
#define MAX 100

int main(void) {
    srand((unsigned)time(NULL));
    int secret = MIN + rand() % (MAX - MIN + 1);
    int guess;
    int tries = 0;

    printf("I'm thinking of a number between %d and %d.\n", MIN, MAX);

    for (;;) {
        printf("Your guess: ");
        if (scanf("%d", &guess) != 1) {
            printf("Please enter a number.\n");
            while (getchar() != '\n')
                ;
            continue;
        }
        tries++;

        if (guess < secret) {
            printf("Too low.\n");
        } else if (guess > secret) {
            printf("Too high.\n");
        } else {
            printf("You got it in %d %s!\n", tries, tries == 1 ? "try" : "tries");
            break;
        }
    }

    return 0;
}
