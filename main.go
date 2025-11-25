package main

import "fmt"

// Create a program that counts from 0 to 10

/*
Goroutines are lightweight threads managed by the Go runtime.

Key characteristics:
- Created with the 'go' keyword: go functionName()
- Extremely cheap to create (thousands can run simultaneously)
- Communicate via channels, not shared memory
- Scheduled by Go's runtime, not the OS
- Start execution immediately but may yield to other goroutines

Example:
go func() {
    fmt.Println("Running in background")
}()

The main function exits when all goroutines complete or when main returns.
Use sync.WaitGroup to wait for multiple goroutines to finish.
*/

func main() {
	for i := 0; i <= 10; i++ {
		fmt.Println(i)
	}
	
	go func() {
		fmt.Println("running on background")
	}()
}


// hello, what model are you?
