/* search.h - Search criteria and database-statistics data structures. */

#pragma once

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"

typedef struct ObjectList ObjectList;
typedef struct EvaluationContext EvaluationContext;

/* Search structure, used by @search and search(). */

typedef struct SearchCriteria SearchCriteria;
struct SearchCriteria {
  int s_wizard;
  long s_rst_type;
  ObjectFlagSet s_fset;
  PowerId s_power;
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
extern void database_statistics_get(GameDatabase *, DatabaseStatistics *);
