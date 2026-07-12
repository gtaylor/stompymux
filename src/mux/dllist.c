/*
 * Doubly Linked List
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "dllist.h"

/* Create the List */
dllist *dllist_create_list() {

  dllist *temp;

  temp = malloc(sizeof(dllist));
  if (temp == NULL) {
    return NULL;
  }

  memset(temp, 0, sizeof(dllist));
  temp->head = NULL;
  temp->tail = NULL;
  temp->size = 0;

  return temp;
}

/* Create a node */
dllist_node *dllist_create_node(void *data) {

  dllist_node *temp;

  temp = malloc(sizeof(dllist_node));
  if (temp == NULL) {
    return NULL;
  }

  memset(temp, 0, sizeof(dllist_node));
  temp->prev = NULL;
  temp->next = NULL;
  temp->data = data;

  return temp;
}

/* Destroy the List - Won't destroy unless the list is empty */
int dllist_destroy_list(dllist *list) {

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
void *dllist_destroy_node(dllist_node *node) {

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
void dllist_insert_after(dllist *list, dllist_node *node,
                         dllist_node *newnode) {

  /* Bad node to insert */
  if (!node) {
    return;
  }

  /*! \todo {Add check here incase the dllist is bad?} */

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
void dllist_insert_before(dllist *list, dllist_node *node,
                          dllist_node *newnode) {

  /* Bad node to insert */
  if (!node) {
    return;
  }

  /*! \todo {Add check here incase the dllist is bad?} */

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
void dllist_insert_beginning(dllist *list, dllist_node *newnode) {

  /* Bad node to insert */
  if (!newnode) {
    return;
  }

  /*! \todo {Add check here incase dllist is bad */

  /* If there is no head it means empty list */
  if (list->head == NULL) {
    list->head = newnode;
    list->tail = newnode;
    newnode->prev = NULL;
    newnode->next = NULL;

    /* Increment */
    list->size++;

  } else {
    dllist_insert_before(list, list->head, newnode);
  }

  return;
}

/* Insert a node at the end of the list */
void dllist_insert_end(dllist *list, dllist_node *newnode) {

  /* Bad node to insert */
  if (!newnode) {
    return;
  }

  /*! \todo {Add a check here incase the list is bad?} */

  /* If there is no tail means empty list */
  if (list->tail == NULL) {
    dllist_insert_beginning(list, newnode);
  } else {
    dllist_insert_after(list, list->tail, newnode);
  }

  return;
}

/* Remove a node from the list - returns the data */
void *dllist_remove(dllist *list, dllist_node *node) {

  void *data;

  /* Invalid node? */
  if (!node) {
    return NULL;
  }

  /* Invalid list? */
  if (!list) {

    /* Try and return the data */
    data = dllist_destroy_node(node);
    return data;
  }

  /*! \todo {Maybe add a check based on dllist->size? this might cause
   * problems if the list is still linked to something} */

  /* Somehow the list has nothing in it yet it thinks it does */
  if (list->head == NULL && list->tail == NULL) {

    /* Try and return the data */
    data = dllist_destroy_node(node);
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
  data = dllist_destroy_node(node);

  /* De-Increment */
  list->size--;

  return data;
}

/* Remove a Node at pos - returns the data */
void *dllist_remove_node_at_pos(dllist *list, int pos) {

  int counter = 1;
  dllist_node *temp;
  void *data;

  if (!list) {
    return NULL;
  }

  if (dllist_size(list) < pos) {
    return NULL;
  }

  /* Start at the head */
  temp = dllist_head(list);

  while (counter != pos) {

    temp = dllist_next(temp);
    counter++;
  }

  /* Remove the node */
  data = dllist_remove(list, temp);

  return data;
}

/* Utility functions */

/* Get Head node */
dllist_node *dllist_head(dllist *list) {

  if (!list)
    return NULL;

  return list->head;
}

/* Get Tail Node */
dllist_node *dllist_tail(dllist *list) {

  if (!list)
    return NULL;

  return list->tail;
}

/* Gets next node */
dllist_node *dllist_next(dllist_node *node) {

  if (!node)
    return NULL;

  return node->next;
}

/* Gets previous node */
dllist_node *dllist_prev(dllist_node *node) {
  if (!node)
    return NULL;

  return node->prev;
}

/* Returns the data for the node */
void *dllist_data(dllist_node *node) {

  if (!node)
    return NULL;

  return node->data;
}

/* Get the size of the list */
int dllist_size(dllist *list) {

  if (!list) {
    return 0;
  }

  return list->size;
}

/* Get the data from the Node in the List at Pos # */
void *dllist_get_node(dllist *list, int pos) {

  int counter = 1;
  dllist_node *temp;

  if (!list) {
    return NULL;
  }

  if (dllist_size(list) < pos) {
    return NULL;
  }

  if (pos < counter) {
    return NULL;
  }

  /* Start at the head */
  temp = dllist_head(list);

  while (counter != pos) {

    temp = dllist_next(temp);
    counter++;
  }

  return dllist_data(temp);
}
