/*
 * Doubly Linked List
 */

/* Doubly Linked List Node */
#pragma once
typedef struct DoublyLinkedListNode {
  struct DoublyLinkedListNode *next, *prev;
  void *data;
} DoublyLinkedListNode;

/* DLLIST - Doesn't store data just
 * size of list and has pointer to
 * head node and tail node */
typedef struct DoublyLinkedList {
  struct DoublyLinkedListNode *head, *tail;
  unsigned int size;
} DoublyLinkedList;

/* The various create functions */
DoublyLinkedList *doubly_linked_list_create_list(void);
DoublyLinkedListNode *doubly_linked_list_create_node(void *data);

/* The different destroy functions
 * destroy_node returns the data */
int doubly_linked_list_destroy_list(DoublyLinkedList *ddlist);
void *doubly_linked_list_destroy_node(
    DoublyLinkedListNode *node); /* Shouldn't include this one but whatever */

/* The various insert functions */
void doubly_linked_list_insert_after(DoublyLinkedList *doubly_linked_list,
                                     DoublyLinkedListNode *node,
                                     DoublyLinkedListNode *newnode);
void doubly_linked_list_insert_before(DoublyLinkedList *doubly_linked_list,
                                      DoublyLinkedListNode *node,
                                      DoublyLinkedListNode *newnode);
void doubly_linked_list_insert_beginning(DoublyLinkedList *doubly_linked_list,
                                         DoublyLinkedListNode *newnode);
void doubly_linked_list_insert_end(DoublyLinkedList *doubly_linked_list,
                                   DoublyLinkedListNode *newnode);

/* Remove nodes and return the data */
void *doubly_linked_list_remove(DoublyLinkedList *doubly_linked_list,
                                DoublyLinkedListNode *node);
void *
doubly_linked_list_remove_node_at_pos(DoublyLinkedList *doubly_linked_list,
                                      int pos);

/* Utility functions */
DoublyLinkedListNode *
doubly_linked_list_head(DoublyLinkedList *doubly_linked_list);
DoublyLinkedListNode *doubly_linked_list_next(DoublyLinkedListNode *node);

void *doubly_linked_list_data(DoublyLinkedListNode *node);
int doubly_linked_list_size(DoublyLinkedList *doubly_linked_list);
void *doubly_linked_list_get_node(DoublyLinkedList *doubly_linked_list,
                                  int pos);
