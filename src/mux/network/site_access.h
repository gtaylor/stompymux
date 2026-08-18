/** @file
 * Network site access and connected-name helpers.
 */
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

/** Executes list siteinfo. @param[in] evaluation Expression evaluation context.
 * @param[in] access_control Access control. @param[in] player Player object. */

void list_siteinfo(EvaluationContext *evaluation,
                   AccessControlStore *access_control, DbRef player);
/** Executes site data check. @param[in] saddr Saddr. @param[in] address_length
 * Address length. @param[in] site_list Site list. */

int site_data_check(struct sockaddr_storage *saddr, int address_length,
                    SiteData *site_list);
/** Executes make ulist. @param[in,out] database Game database. @param[in,out]
 * descriptors Descriptors. @param[in] player Player object. @param[out] buffer
 * Caller-owned output storage. @param[in,out] bufc Bufc. */

void make_ulist(GameDatabase *database, DescriptorRegistry *descriptors,
                DbRef player, char *buffer, char **bufc);
/** Finds find connected name. @param[in] database Game database. @param[in]
 * descriptors Descriptors. @param[in] player Player object. @param[in] name
 * Name to use. */

DbRef find_connected_name(GameDatabase *database,
                          DescriptorRegistry *descriptors, DbRef player,
                          const char *name);
/** Executes descriptor run command. @param[in,out] descriptor Network
 * descriptor. @param[in,out] command Command text or descriptor. */

void descriptor_run_command(Descriptor *descriptor, char *command);
