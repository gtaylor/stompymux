
/*
 * Author: Markus "iDLari" Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Thu Aug 29 09:51:22 1996 fingon
 * Last modified: Fri Nov 27 20:15:14 1998 fingon
 *
 */

/* mux_event_alloc.h - Allocation convenience macros for event clients. */

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
