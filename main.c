#include <stdio.h>
#include <stdlib.h>

typedef struct Node {
    int data;
    struct Node *next;
} Node;

Node* create_node(int data) {
    Node *new_node = malloc(sizeof(Node));
    if (!new_node) {
        perror("malloc failed");
        exit(EXIT_FAILURE);
    }
    new_node->data = data;
    new_node->next = NULL;
    return new_node;
}

void prepend(Node **head, int data) {
    Node *new_node = create_node(data);
    new_node->next = *head;
    *head = new_node;
}

void append(Node **head, int data) {
    Node *new_node = create_node(data);
    if (*head == NULL) {
        *head = new_node;
        return;
    }
    Node *curr = *head;
    while (curr->next != NULL) {
        curr = curr->next;
    }
    curr->next = new_node;
}

int delete_by_value(Node **head, int value) {
    if (*head == NULL) return 0;

    Node *curr = *head;
    Node *prev = NULL;

    while (curr != NULL && curr->data != value) {
        prev = curr;
        curr = curr->next;
    }

    if (curr == NULL) return 0;

    if (prev == NULL) {
        *head = curr->next;
    } else {
        prev->next = curr->next;
    }

    free(curr);
    return 1;
}

void print_list(const Node *head) {
    printf("[");
    const Node *curr = head;
    while (curr != NULL) {
        printf("%d", curr->data);
        if (curr->next != NULL) printf(", ");
        curr = curr->next;
    }
    printf("]\n");
}

void free_list(Node *head) {
    Node *curr = head;
    while (curr != NULL) {
        Node *tmp = curr;
        curr = curr->next;
        free(tmp);
    }
}

size_t length(const Node *head) {
    size_t count = 0;
    const Node *curr = head;
    while (curr != NULL) {
        count++;
        curr = curr->next;
    }
    return count;
}

int main(void) {
    Node *list = NULL;

    append(&list, 10);
    append(&list, 20);
    append(&list, 30);
    prepend(&list, 5);

    printf("List after inserts: ");
    print_list(list);
    printf("Length: %zu\n", length(list));

    printf("Deleting 20...\n");
    if (delete_by_value(&list, 20)) {
        printf("List after delete: ");
        print_list(list);
    } else {
        printf("Value 20 not found.\n");
    }

    printf("Deleting 99 (not in list)...\n");
    if (!delete_by_value(&list, 99)) {
        printf("Value 99 not found.\n");
    }

    free_list(list);
    return 0;
}
