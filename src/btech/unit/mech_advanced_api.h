/* Declares the BattleTech unit advanced API. */

#pragma once

#include "mux/server/platform.h"

/* mech.advanced.c */
void mech_ecm(DbRef player, Mech *mech, char *buffer);
void mech_eccm(DbRef player, Mech *mech, char *buffer);
void mech_perecm(DbRef player, Mech *mech, char *buffer);
void mech_pereccm(DbRef player, Mech *mech, char *buffer);
void mech_angelecm(DbRef player, Mech *mech, char *buffer);
void mech_angeleccm(DbRef player, Mech *mech, char *buffer);
void mech_stinger(DbRef player, Mech *mech, char *buffer);
void mech_slite(DbRef player, Mech *mech, char *buffer);
void mech_ams(DbRef player, Mech *mech, char *buffer);
void mech_fliparms(DbRef player, Mech *mech, char *buffer);
void mech_flamerheat(DbRef player, Mech *mech, char *buffer);
void mech_ultra(DbRef player, Mech *mech, char *buffer);
void mech_rac(DbRef player, Mech *mech, char *buffer);
void mech_rapidfire(DbRef player, Mech *mech, char *buffer);
void mech_unjamammo(DbRef player, Mech *mech, char *buffer);
void mech_gattling(DbRef player, Mech *mech, char *buffer);
void mech_inarc_ammo_toggle(DbRef player, Mech *mech, char *buffer);
void mech_explosive(DbRef player, Mech *mech, char *buffer);
void mech_lbx(DbRef player, Mech *mech, char *buffer);
void mech_armorpiercing(DbRef player, Mech *mech, char *buffer);
void mech_flechette(DbRef player, Mech *mech, char *buffer);
void mech_incendiary(DbRef player, Mech *mech, char *buffer);
void mech_precision(DbRef player, Mech *mech, char *buffer);
void mech_caseless(DbRef player, Mech *mech, char *buffer);
void mech_artemis(DbRef player, Mech *mech, char *buffer);
void mech_narc(DbRef player, Mech *mech, char *buffer);
void mech_swarm(DbRef player, Mech *mech, char *buffer);
void mech_swarm1(DbRef player, Mech *mech, char *buffer);
void mech_inferno(DbRef player, Mech *mech, char *buffer);
void mech_hotload(DbRef player, Mech *mech, char *buffer);
void mech_sguided(DbRef player, Mech *mech, char *buffer);
void mech_atmrange(DbRef player, Mech *mech, char *buffer);
void mech_atmexplosive(DbRef player, Mech *mech, char *buffer);
void mech_cluster(DbRef player, Mech *mech, char *buffer);
void mech_smoke(DbRef player, Mech *mech, char *buffer);
void mech_mine(DbRef player, Mech *mech, char *buffer);
void mech_masc(DbRef player, Mech *mech, char *buffer);
void mech_scharge(DbRef player, Mech *mech, char *buffer);
void mech_explode(DbRef player, Mech *mech, char *buffer);
void mech_dig(DbRef player, Mech *mech, char *buffer);
void mech_fixturret(DbRef player, Mech *mech, char *buffer);
void mech_disableweap(DbRef player, Mech *mech, char *buffer);
int find_main_weapon(Mech *mech, int (*callback)(Mech *, int, int, int, int));
void mech_stealtharmor(DbRef player, Mech *mech, char *buffer);
void mech_nullsig(DbRef player, Mech *mech, char *buffer);
void show_narc_pods(DbRef player, Mech *mech, char *buffer);
void remove_inarc_pods_mech(DbRef player, Mech *mech, char *buffer);
void remove_inarc_pods_tank(DbRef player, Mech *mech, char *buffer);
void mech_auto_turret(DbRef player, Mech *mech, char *buffer);
void mech_usebin(DbRef player, Mech *mech, char *buffer);
void mech_safety(DbRef player, Mech *mech, char *buffer);
void mech_mechprefs(DbRef player, Mech *mech, char *buffer);
