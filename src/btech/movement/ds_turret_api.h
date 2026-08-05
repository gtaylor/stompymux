#pragma once

#include "mux/server/platform.h"

void turret_addtic(DbRef player, void *data, char *buffer);
void turret_deltic(DbRef player, void *data, char *buffer);
void turret_listtic(DbRef player, void *data, char *buffer);
void turret_cleartic(DbRef player, void *data, char *buffer);
void turret_firetic(DbRef player, void *data, char *buffer);
void turret_bearing(DbRef player, void *data, char *buffer);
void turret_eta(DbRef player, void *data, char *buffer);
void turret_findcenter(DbRef player, void *data, char *buffer);
void turret_fireweapon(DbRef player, void *data, char *buffer);
void turret_settarget(DbRef player, void *data, char *buffer);
void turret_lrsmap(DbRef player, void *data, char *buffer);
void turret_navigate(DbRef player, void *data, char *buffer);
void turret_range(DbRef player, void *data, char *buffer);
void turret_sight(DbRef player, void *data, char *buffer);
void turret_tacmap(DbRef player, void *data, char *buffer);
void turret_contacts(DbRef player, void *data, char *buffer);
void turret_critstatus(DbRef player, void *data, char *buffer);
void turret_report(DbRef player, void *data, char *buffer);
void turret_scan(DbRef player, void *data, char *buffer);
void turret_status(DbRef player, void *data, char *buffer);
void turret_weaponspecs(DbRef player, void *data, char *buffer);
void turret_lifecycle_update(DbRef key, void **data, int selector);
void turret_initialize(DbRef player, void *data, char *buffer);
void turret_deinitialize(DbRef player, void *data, char *buffer);
