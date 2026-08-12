/* Network site access and connected-name helpers. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_registries.h"

struct sockaddr_storage;

typedef struct AccessControlStore AccessControlStore;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct SiteData SiteData;

void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player);
int site_data_check(struct sockaddr_storage *saddr, int address_length,
                    SiteData *site_list);
void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buffer, char **bufc);
DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          const char *name);
void descriptor_run_command(Descriptor *descriptor, char *command);
