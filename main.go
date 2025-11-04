package main

import "fmt"

func main() {
	fmt.Println("hello world")

	a := hello_world()

	fmt.Println(a)
}

func hello_world() int {
	return 1 + 1 
}
