package main

import "fmt"

func main() {
	a, err := fmt.Print("hello fellas")

	fmt.Println(a, err)

	result := hello()
	fmt.Println(result)
}

func hello(a int, b int, c int) int {
	return a + b
}
