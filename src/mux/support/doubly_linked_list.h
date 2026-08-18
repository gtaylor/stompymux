/** @file
 * Doubly Linked List.
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
/** Executes doubly linked list create list. */

DoublyLinkedList *doubly_linked_list_create_list(void);
/** Executes doubly linked list create node. @param[in] data Caller-provided
 * data. */

DoublyLinkedListNode *doubly_linked_list_create_node(void *data);

/* The different destroy functions
 * destroy_node returns the data */
/** Executes doubly linked list destroy list. @param[in,out] ddlist Ddlist. */

bool doubly_linked_list_destroy_list(DoublyLinkedList *ddlist);
/** Executes doubly linked list destroy node. @param[in,out] node Node. */

void *doubly_linked_list_destroy_node(
    DoublyLinkedListNode *node); /* Shouldn't include this one but whatever */

/* The various insert functions */
/** Executes doubly linked list insert after. @param[in,out] doubly_linked_list
 * Doubly linked list. @param[in,out] node Node. @param[in,out] newnode Newnode.
 */

void doubly_linked_list_insert_after(DoublyLinkedList *doubly_linked_list,
                                     DoublyLinkedListNode *node,
                                     DoublyLinkedListNode *newnode);
/** Executes doubly linked list insert before. @param[in,out] doubly_linked_list
 * Doubly linked list. @param[in,out] node Node. @param[in,out] newnode Newnode.
 */

void doubly_linked_list_insert_before(DoublyLinkedList *doubly_linked_list,
                                      DoublyLinkedListNode *node,
                                      DoublyLinkedListNode *newnode);
/** Executes doubly linked list insert beginning. @param[in,out]
 * doubly_linked_list Doubly linked list. @param[in,out] newnode Newnode. */

void doubly_linked_list_insert_beginning(DoublyLinkedList *doubly_linked_list,
                                         DoublyLinkedListNode *newnode);
/** Executes doubly linked list insert end. @param[in,out] doubly_linked_list
 * Doubly linked list. @param[in,out] newnode Newnode. */

void doubly_linked_list_insert_end(DoublyLinkedList *doubly_linked_list,
                                   DoublyLinkedListNode *newnode);

/* Remove nodes and return the data */
/** Removes doubly linked list. @param[in,out] doubly_linked_list Doubly linked
 * list. @param[in,out] node Node. */

void *doubly_linked_list_remove(DoublyLinkedList *doubly_linked_list,
                                DoublyLinkedListNode *node);
/** Removes node at pos from doubly linked list. @param[in,out]
 * doubly_linked_list Doubly linked list. @param[in] pos Pos. */

void *
doubly_linked_list_remove_node_at_pos(DoublyLinkedList *doubly_linked_list,
                                      int pos);

/* Utility functions */
/** Executes doubly linked list head. @param[in,out] doubly_linked_list Doubly
 * linked list. */

DoublyLinkedListNode *
doubly_linked_list_head(DoublyLinkedList *doubly_linked_list);
/** Executes doubly linked list next. @param[in,out] node Node. */

DoublyLinkedListNode *doubly_linked_list_next(DoublyLinkedListNode *node);

/** Executes doubly linked list data. @param[in,out] node Node. */

void *doubly_linked_list_data(DoublyLinkedListNode *node);
/** Executes doubly linked list size. @param[in] doubly_linked_list Doubly
 * linked list. */

int doubly_linked_list_size(DoublyLinkedList *doubly_linked_list);
/** Returns node from doubly linked list. @param[in,out] doubly_linked_list
 * Doubly linked list. @param[in] pos Pos. */

void *doubly_linked_list_get_node(DoublyLinkedList *doubly_linked_list,
                                  int pos);
