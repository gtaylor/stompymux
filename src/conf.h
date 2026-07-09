#pragma once

#include "db.h"

void cf_init(void);
void cf_log_notfound(dbref player, char *cmd, const char *thingname,
                     char *thing);
void cf_log_syntax(dbref player, char *cmd, const char *template, char *arg);
void list_cf_access(dbref player);
int cf_read(char *fn);
int cf_set(char *cp, char *ap, dbref player);
int cf_modify_bits(int *vp, char *str, long extra, dbref player, char *cmd);
