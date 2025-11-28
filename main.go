package main

import (
	"bufio"
	"fmt"
	"log"
	"math/rand"
	"os"
	"strconv"
	"strings"
	"time"
)

func main() {
	rand.Seed(time.Now().UnixNano())
	secret := rand.Intn(100) + 1
	guesses := 0
	maxGuesses := 7

	fmt.Println("=== NUMBER GUESSING GAME ===")
	fmt.Printf("I'm thinking of a number between 1 and 100.\nYou have %d guesses to find it!\n\n", maxGuesses)

	scanner := bufio.NewScanner(os.Stdin)

	for guesses < maxGuesses {
		fmt.Print("Enter your guess: ")
		if !scanner.Scan() {
			log.Fatal("Failed to read input")
		}

		input := strings.TrimSpace(scanner.Text())
		guess, err := strconv.Atoi(input)
		if err != nil {
			fmt.Println("❌ Please enter a valid number.")
			continue
		}

		if guess < 1 || guess > 100 {
			fmt.Println("❌ Please guess a number between 1 and 100.")
			continue
		}

		guesses++
		remaining := maxGuesses - guesses

		if guess == secret {
			fmt.Printf("\n🎉 You got it! The number was %d!\n", secret)
			fmt.Printf("You won in %d guess(es)!\n", guesses)
			return
		} else if guess < secret {
			fmt.Printf("📈 Too low! (%d guesses left)\n\n", remaining)
		} else {
			fmt.Printf("📉 Too high! (%d guesses left)\n\n", remaining)
		}
	}

	fmt.Printf("\n💀 Game Over! The number was %d.\n", secret)
}
