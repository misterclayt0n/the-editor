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
	reader := bufio.NewReader(os.Stdin)

	fmt.Println("Guess the number between 1 and 100!")

	for {
		fmt.Print("Enter your guess: ")
		input, _ := reader.ReadString('\n')
		input = strings.TrimSpace(input)
		guess, err := strconv.Atoi(input)
		if err != nil {
			fmt.Println("Please enter a valid number.")
			continue
		}

		if guess < target {
			fmt.Println("Too low!")
		} else if guess > target {
			fmt.Println("Too high!")
		} else {
			fmt.Println("Correct! You guessed it.")
			break
		}
	}
}

// hello world
// we gucci
//
//
//
//
// write yay in the end of this file
