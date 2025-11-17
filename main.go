package main

func main() {
	a, err := fmt.Print("hello fellas")

	fmt.Println(a, err)

	result := hello(1, 2, 3)
	fmt.Println(result)
}

func hello(a int, b int, c int) int {
	return a + b + c
}
