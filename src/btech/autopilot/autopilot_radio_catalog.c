
/*
 * $Id: autopilot_command.c,v 1.4 2005/08/10 14:09:34 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Tue Sep 23 20:33:33 1997 fingon
 * Last modified: Sat Jun  6 21:47:38 1998 fingon
 *
 */

/* Most of the BattleSheep(tm) code is here.. */

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "autopilot.h"
#include "autopilot_radio_internal.h"
#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_sensor_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"

void sendchannelstuff(Mech *mech, int freq, char *msg);
/*
 * Master list of AI - radio commands
 */
AutopilotRadioCommand const autopilot_radio_commands[] = {
    {"auto", "autogun", 1, 0, auto_radio_command_autogun},
    {"auto", "autogun", 2, 0, auto_radio_command_autogun},
    {
#if 0
	"att", "attackleg", 1, 0, auto_attackleg}, {
	"chanf", "chanfreq", 2, 0, auto_setchanfreq}, {
	"chanm", "chanmode", 2, 0, auto_setchanmode}, {
#endif
        "chase", "chasetarg", 1, 0, auto_radio_command_chasetarg},
    {
#if 0
	"cm", "cmode", 2, 0, auto_cmode}, {
#endif
        "dfo", "dfollow", 1, 0, auto_radio_command_dfollow},
    {"dgo", "dgoto", 2, 0, auto_radio_command_dgoto},
    {
#if 0
	"dr", "drally", 2, 0, auto_drally}, {
	"dr", "drally", 3, 0, auto_drally}, {
#endif
        "drop", "dropoff", 0, 0, auto_radio_command_dropoff},
    {"emb", "embark", 1, 0, auto_radio_command_embark},
    {"en", "enterbase", 0, 0, auto_radio_command_enterbase},
    {"en", "enterbase", 1, 0, auto_radio_command_enterbase},
    {
#if 0
	"en", "enterbay", 0, 0, auto_enterbay}, {
	"en", "enterbay", 1, 0, auto_enterbay}, {
#endif
        "fo", "follow", 1, 0, auto_radio_command_follow},
    {
#if 0
	"fr", "freq", 1, 0, auto_freq}, {
#endif
        "go", "goto", 2, 0, auto_radio_command_goto},
    {"he", "heading", 1, 0, auto_radio_command_heading},
    {"he", "help", 0, 1, auto_radio_command_help},
    {"hi", "hide", 0, 0, auto_radio_command_hide},
    {"jump", "jumpjet", 1, 0, auto_radio_command_jumpjet},
    {"jump", "jumpjet", 2, 0, auto_radio_command_jumpjet},
    {"le", "leavebase", 1, 0, auto_radio_command_leavebase},
    {
#if 0
	"nog", "nogun", 0, 0, auto_nogun}, {
	"not", "notarget", 0, 0, auto_notarget}, {
#endif
        "ogo", "ogoto", 2, 0, auto_radio_command_ogoto},
    {"pick", "pickup", 1, 0, auto_radio_command_pickup},
    {"pos", "position", 2, 0, auto_radio_command_position},
    {"pr", "prone", 0, 0, auto_radio_command_prone},
    {
#if 0
	"ra", "rally", 2, 0, auto_rally}, {
	"ra", "rally", 3, 0, auto_rally}, {
#endif
        "re", "report", 0, 1, auto_radio_command_report},
    {"reset", "reset", 0, 0, auto_radio_command_reset},
    {
#if 0
	"roam", "roammode", 1, 0, auto_roammode}, {
#endif
        "se", "sensor", 2, 0, auto_radio_command_sensor},
    {"se", "sensor", 0, 0, auto_radio_command_sensor},
    {"sh", "shutdown", 0, 0, auto_radio_command_shutdown},
    {"sp", "speed", 1, 0, auto_radio_command_speed},
    {"st", "stand", 0, 0, auto_radio_command_stand},
    {"st", "startup", 0, 0, auto_radio_command_startup},
    {"st", "startup", 1, 0, auto_radio_command_startup},
    {"st", "stop", 0, 0, auto_radio_command_stop},
    {"sw", "sweight", 2, 1, auto_radio_command_sweight},
    {
#if 0
	"swa", "swarm", 1, 0, auto_swarm}, {
	"swarmc", "swarmcharge", 1, 0, auto_swarmcharge}, {
	"swarmm", "swarmmode", 1, 0, auto_swarmmode}, {
#endif
        "ta", "target", 1, 0, auto_radio_command_target},
    {
#if 0
	"ta", "target", 2, 0, auto_target}, {
#endif
        NULL, NULL, 0, 0, NULL}};
