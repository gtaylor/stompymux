/* Network site access and connected-name helpers. */

#pragma once

#include <netinet/in.h>

#include "mux/network/descriptor.h"
#include "mux/objects/db.h"

typedef struct AccessControlStore AccessControlStore;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct SiteData SiteData;

void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player);
int site_data_check(struct sockaddr_storage *address, int address_length,
                    SiteData *site_list);
void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buffer, char **cursor);
DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          char *name);
void descriptor_run_command(Descriptor *descriptor, char *command);
