package main

import "fmt"

type nothing struct {
	Name string
}

func (g nothing ) Message() string {
	return fmt.Sprintf("hello, %s", g.Name)
}

func NewGreeter(name string) nothing {
	return nothing {Name: name}
}

func main() {
	greeter := NewGreeter("lsp")
	fmt.Println(greeter.Message())
}
