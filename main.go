package main

import (
	"fmt"
	"math/rand"
	"time"
)

func main() {
	// Seed the random number generator
	rand.Seed(time.Now().UnixNano())

	// Generate a random number between 1 and 100
	secretNumber := rand.Intn(100) + 1

	fmt.Println("I'm thinking of a number between 1 and 100. Can you guess what it is?")

	for {
		var guess int
		fmt.Print("Your guess: ")
		_, err := fmt.Scanln(&guess)
		if err != nil {
			fmt.Println("Invalid input. Please enter a number.")
			continue
		}

		if guess < secretNumber {
			fmt.Println("Too low!")
		} else if guess > secretNumber {
			fmt.Println("Too high!")
		} else {
			fmt.Println("Correct! You guessed the number.")
			break
		}
	}
}
