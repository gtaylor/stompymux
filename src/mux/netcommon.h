#pragma once

#include "db.h"
#include "interface.h"

void make_portlist(dbref player, dbref target, char *buff, char **bufc);
struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
void raw_broadcast(int inflags, char *template, ...);
void run_command(DESC *d, char *command);
int do_command(DESC *d, char *command);
int do_unauth_command(DESC *d, char *command);
void handle_prog(DESC *d, char *message);
void desc_reload(dbref player);
void init_logout_cmdtab(void);
void list_siteinfo(dbref player);
int fetch_idle(dbref target);
int fetch_connect(dbref target);
void make_ulist(dbref player, char *buff, char **bufc);
