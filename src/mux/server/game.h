/* game.h - Core notifications, database dumps, and shutdown interface. */

#pragma once

#include "mux/database/db.h"

void do_shutdown(DbRef player, DbRef cause, int key, char *message);
void notify_except(DbRef location, DbRef player, DbRef exception,
                   const char *message);
void notify_except2(DbRef location, DbRef player, DbRef exception1,
                    DbRef exception2, const char *message);
void notify_printf(DbRef player, const char *format, ...)
    __attribute__((format(printf, 2, 3)));
int check_filter(DbRef object, DbRef player, int filter, const char *message);
void notify_checked(DbRef target, DbRef sender, const char *message, int key);
int is_hearer(DbRef object);
void report(void);
int attribute_match(DbRef thing, DbRef player, char type, const char *string,
                    int check_parent);
int list_check(DbRef player, DbRef thing, char type, char *string,
               int check_parent);
