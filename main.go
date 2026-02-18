package main

import "fmt"

func main() {
	yes, err := fmt.Println("hi fellas")
	if err != nil {
		fmt.Errorf(err.Error())
	}

	fmt.Println(yes)
}
