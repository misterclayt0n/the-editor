package main

import (
	"bufio"
	"fmt"
	"math/rand"
	"os"
	"strconv"
	"strings"
	"time"
)

func main() {
	rand.Seed(time.Now().UnixNano())
	target := rand.Intn(100) + 1
	attempts := 0
	maxAttempts := 7

	fmt.Println("Welcome to the Number Guessing Game!")
	fmt.Printf("I'm thinking of a number between 1 and 100.\n")
	fmt.Printf("You have %d attempts to guess it.\n\n", maxAttempts)

	scanner := bufio.NewScanner(os.Stdin)

	for attempts < maxAttempts {
		fmt.Printf("Attempt %d/%d - Enter your guess: ", attempts+1, maxAttempts)

		if !scanner.Scan() {
			break
		}

		input := strings.TrimSpace(scanner.Text())
		guess, err := strconv.Atoi(input)

		if err != nil {
			fmt.Println("Invalid input! Please enter a number between 1 and 100.")
			continue
		}

		if guess < 1 || guess > 100 {
			fmt.Println("Please enter a number between 1 and 100.")
			continue
		}

		attempts++

		if guess == target {
			fmt.Printf("\nCongratulations! You guessed the number in %d attempts!\n", attempts)
			return
		} else if guess < target {
			fmt.Println("Too low! Try a higher number.")
		} else {
			fmt.Println("Too high! Try a lower number.")
		}
		fmt.Println()
	}

	fmt.Printf("\nGame Over! You've used all %d attempts.\n", maxAttempts)
	fmt.Printf("The number was: %d\n", target)
}
