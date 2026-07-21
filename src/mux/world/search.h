/* search.h - Search criteria and database-statistics data structures. */

#pragma once

#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"

typedef struct ObjectList ObjectList;
typedef struct EvaluationContext EvaluationContext;

/* Search structure, used by @search and search(). */

typedef struct SearchCriteria SearchCriteria;
struct SearchCriteria {
  int s_wizard;
  DbRef s_owner;
  DbRef s_rst_owner;
  long s_rst_type;
  FLAGSET s_fset;
  POWERSET s_pset;
  DbRef s_zone;
  char *s_rst_name;
  long low_bound;
  long high_bound;
};

/* Stats structure, used by @stats and stats(). */

typedef struct DatabaseStatistics DatabaseStatistics;
struct DatabaseStatistics {
  int s_total;
  int s_rooms;
  int s_exits;
  int s_things;
  int s_players;
  int s_garbage;
};

extern int search_criteria_setup(EvaluationContext *context, DbRef player,
                                 char *search, SearchCriteria *criteria);
extern void search_criteria_perform(EvaluationContext *context, DbRef player,
                                    DbRef cause, SearchCriteria *criteria,
                                    ObjectList *results);
extern int database_statistics_get(EvaluationContext *, DbRef, DbRef,
                                   DatabaseStatistics *);
