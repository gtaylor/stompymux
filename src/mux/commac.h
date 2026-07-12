
/* commac.h */

/* $Id: commac.h,v 1.1 2005/06/13 20:50:46 murrayma Exp $ */

#pragma once

#include "config.h"

struct commac {
  dbref who;

  int numchannels;
  int maxchannels;
  char *alias;
  char **channels;

  int curmac;
  int macros[5];

  struct commac *next;
};

#define NUM_COMMAC 500

extern struct commac *commac_table[NUM_COMMAC];

void purge_commac(void);

void sort_com_aliases(struct commac *c);
struct commac *get_commac(dbref which);
struct commac *create_new_commac(void);
void destroy_commac(struct commac *c);
void add_commac(struct commac *c);
void del_commac(dbref who);
