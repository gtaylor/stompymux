
/* Defines allocation convenience macros for timed-event clients. */

#pragma once

#define Create(a, b, c)                                                        \
  if (!((a) = (b *)calloc(sizeof(b), c))) {                                    \
    printf("Unable to malloc!\n");                                             \
    exit(1);                                                                   \
  }

#define MyReCreate(a, b, c)                                                    \
  if (!((a) = (b *)realloc((void *)a, sizeof(b) * (c)))) {                     \
    printf("Unable to realloc!\n");                                            \
    exit(1);                                                                   \
  }

#define ReCreate(a, b, c)                                                      \
  if (a) {                                                                     \
    MyReCreate(a, b, c);                                                       \
  } else {                                                                     \
    Create(a, b, c);                                                           \
  }
