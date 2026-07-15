/*
 * Doubly Linked List
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/doubly_linked_list.h"

/* Create the List */
DoublyLinkedList *doubly_linked_list_create_list() {

  DoublyLinkedList *temp;

  temp = malloc(sizeof(DoublyLinkedList));
  if (temp == NULL) {
    return NULL;
  }

  memset(temp, 0, sizeof(DoublyLinkedList));
  temp->head = NULL;
  temp->tail = NULL;
  temp->size = 0;

  return temp;
}

/* Create a node */
DoublyLinkedListNode *doubly_linked_list_create_node(void *data) {

  DoublyLinkedListNode *temp;

  temp = malloc(sizeof(DoublyLinkedListNode));
  if (temp == NULL) {
    return NULL;
  }

  memset(temp, 0, sizeof(DoublyLinkedListNode));
  temp->prev = NULL;
  temp->next = NULL;
  temp->data = data;

  return temp;
}

/* Destroy the List - Won't destroy unless the list is empty */
int doubly_linked_list_destroy_list(DoublyLinkedList *list) {

  if (!list)
    return 1;

  /* Check the size */
  if (list->size != 0) {
    return 0;
  } else {
    list->head = NULL;
    list->tail = NULL;
    free(list);
    return 1;
  }
}

/* Destroy a Node - Returns the data in the node */
void *doubly_linked_list_destroy_node(DoublyLinkedListNode *node) {

  void *data;

  if (!node)
    return NULL;

  data = node->data;
  node->prev = NULL;
  node->next = NULL;
  node->data = NULL;
  free(node);

  return data;
}

/* Insert a given node after a node */
void doubly_linked_list_insert_after(DoublyLinkedList *list,
                                     DoublyLinkedListNode *node,
                                     DoublyLinkedListNode *newnode) {

  /* Bad node to insert */
  if (!node) {
    return;
  }

  /*! \todo {Add check here incase the DoublyLinkedList is bad?} */

  newnode->prev = node;
  newnode->next = node->next;
  if (node->next == NULL) {
    list->tail = newnode;
  } else {
    (node->next)->prev = newnode;
  }
  node->next = newnode;

  /* Increment */
  list->size++;

  return;
}

/* Insert a given node before a node */
void doubly_linked_list_insert_before(DoublyLinkedList *list,
                                      DoublyLinkedListNode *node,
                                      DoublyLinkedListNode *newnode) {

  /* Bad node to insert */
  if (!node) {
    return;
  }

  /*! \todo {Add check here incase the DoublyLinkedList is bad?} */

  newnode->prev = node->prev;
  newnode->next = node;
  if (node->prev == NULL) {
    list->head = newnode;
  } else {
    (node->prev)->next = newnode;
  }
  node->prev = newnode;

  /* Increment */
  list->size++;

  return;
}

/* Insert a node at the beginning */
void doubly_linked_list_insert_beginning(DoublyLinkedList *list,
                                         DoublyLinkedListNode *newnode) {

  /* Bad node to insert */
  if (!newnode) {
    return;
  }

  /*! \todo {Add check here incase DoublyLinkedList is bad */

  /* If there is no head it means empty list */
  if (list->head == NULL) {
    list->head = newnode;
    list->tail = newnode;
    newnode->prev = NULL;
    newnode->next = NULL;

    /* Increment */
    list->size++;

  } else {
    doubly_linked_list_insert_before(list, list->head, newnode);
  }

  return;
}

/* Insert a node at the end of the list */
void doubly_linked_list_insert_end(DoublyLinkedList *list,
                                   DoublyLinkedListNode *newnode) {

  /* Bad node to insert */
  if (!newnode) {
    return;
  }

  /*! \todo {Add a check here incase the list is bad?} */

  /* If there is no tail means empty list */
  if (list->tail == NULL) {
    doubly_linked_list_insert_beginning(list, newnode);
  } else {
    doubly_linked_list_insert_after(list, list->tail, newnode);
  }

  return;
}

/* Remove a node from the list - returns the data */
void *doubly_linked_list_remove(DoublyLinkedList *list,
                                DoublyLinkedListNode *node) {

  void *data;

  /* Invalid node? */
  if (!node) {
    return NULL;
  }

  /* Invalid list? */
  if (!list) {

    /* Try and return the data */
    data = doubly_linked_list_destroy_node(node);
    return data;
  }

  /*! \todo {Maybe add a check based on DoublyLinkedList->size? this might cause
   * problems if the list is still linked to something} */

  /* Somehow the list has nothing in it yet it thinks it does */
  if (list->head == NULL && list->tail == NULL) {

    /* Try and return the data */
    data = doubly_linked_list_destroy_node(node);
    return data;
  }

  /* We're checking if this first node */
  if (node->prev == NULL) {
    list->head = node->next;
  } else {
    (node->prev)->next = node->next;
  }

  /* Check if end of list */
  if (node->next == NULL) {
    list->tail = node->prev;
  } else {
    (node->next)->prev = node->prev;
  }

  /* Destroy Node */
  data = doubly_linked_list_destroy_node(node);

  /* De-Increment */
  list->size--;

  return data;
}

/* Remove a Node at pos - returns the data */
void *doubly_linked_list_remove_node_at_pos(DoublyLinkedList *list, int pos) {

  int counter = 1;
  DoublyLinkedListNode *temp;
  void *data;

  if (!list) {
    return NULL;
  }

  if (doubly_linked_list_size(list) < pos) {
    return NULL;
  }

  /* Start at the head */
  temp = doubly_linked_list_head(list);

  while (counter != pos) {

    temp = doubly_linked_list_next(temp);
    counter++;
  }

  /* Remove the node */
  data = doubly_linked_list_remove(list, temp);

  return data;
}

/* Utility functions */

/* Get Head node */
DoublyLinkedListNode *doubly_linked_list_head(DoublyLinkedList *list) {

  if (!list)
    return NULL;

  return list->head;
}

/* Gets next node */
DoublyLinkedListNode *doubly_linked_list_next(DoublyLinkedListNode *node) {

  if (!node)
    return NULL;

  return node->next;
}

/* Returns the data for the node */
void *doubly_linked_list_data(DoublyLinkedListNode *node) {

  if (!node)
    return NULL;

  return node->data;
}

/* Get the size of the list */
int doubly_linked_list_size(DoublyLinkedList *list) {

  if (!list) {
    return 0;
  }

  return list->size;
}

/* Get the data from the Node in the List at Pos # */
void *doubly_linked_list_get_node(DoublyLinkedList *list, int pos) {

  int counter = 1;
  DoublyLinkedListNode *temp;

  if (!list) {
    return NULL;
  }

  if (doubly_linked_list_size(list) < pos) {
    return NULL;
  }

  if (pos < counter) {
    return NULL;
  }

  /* Start at the head */
  temp = doubly_linked_list_head(list);

  while (counter != pos) {

    temp = doubly_linked_list_next(temp);
    counter++;
  }

  return doubly_linked_list_data(temp);
}
