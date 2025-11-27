package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode"
)

func caesarEncrypt(text string, shift int) string {
	var result strings.Builder

	for _, char := range text {
		if unicode.IsUpper(char) {
			result.WriteRune(rune((int(char-'A')+shift)%26 + int('A')))
		} else if unicode.IsLower(char) {
			result.WriteRune(rune((int(char-'a')+shift)%26 + int('a')))
		} else {
			result.WriteRune(char)
		}
	}

	return result.String()
}

func caesarDecrypt(text string, shift int) string {
	return caesarEncrypt(text, 26-shift)
}

func main() {
	reader := bufio.NewReader(os.Stdin)

	fmt.Println("Welcome to the Caesar Cipher!")
	fmt.Println()

	for {
		fmt.Print("Choose an option:\n1. Encrypt\n2. Decrypt\n3. Exit\nEnter choice: ")
		choice, _ := reader.ReadString('\n')
		choice = strings.TrimSpace(choice)

		if choice == "3" {
			fmt.Println("Goodbye!")
			break
		}

		if choice != "1" && choice != "2" {
			fmt.Println("Invalid choice. Please try again.\n")
			continue
		}

		fmt.Print("Enter text: ")
		text, _ := reader.ReadString('\n')
		text = strings.TrimSpace(text)

		fmt.Print("Enter shift value (1-25): ")
		shiftStr, _ := reader.ReadString('\n')
		shiftStr = strings.TrimSpace(shiftStr)

		shift, err := strconv.Atoi(shiftStr)
		if err != nil || shift < 1 || shift > 25 {
			fmt.Println("Invalid shift value. Please enter a number between 1 and 25.\n")
			continue
		}

		if choice == "1" {
			encrypted := caesarEncrypt(text, shift)
			fmt.Printf("Encrypted: %s\n\n", encrypted)
		} else {
			decrypted := caesarDecrypt(text, shift)
			fmt.Printf("Decrypted: %s\n\n", decrypted)
		}
	}
}
