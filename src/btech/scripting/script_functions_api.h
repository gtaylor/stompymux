#pragma once

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

typedef void BtechScriptFunction(char *buff, char **bufc, DbRef player,
                                 DbRef cause, char *fargs[], int nfargs,
                                 char *cargs[], int ncargs,
                                 EvaluationContext *context);

extern BtechScriptFunction fun_btaddstores;
extern BtechScriptFunction fun_btarmorstatus;
extern BtechScriptFunction fun_btarmorstatus_ref;
extern BtechScriptFunction fun_btcharlist;
extern BtechScriptFunction fun_btcritslot;
extern BtechScriptFunction fun_btcritslot_ref;
extern BtechScriptFunction fun_btcritstatus;
extern BtechScriptFunction fun_btcritstatus_ref;
extern BtechScriptFunction fun_btdamagemech;
extern BtechScriptFunction fun_btdamages;
extern BtechScriptFunction fun_btdesignex;
extern BtechScriptFunction fun_btengrate;
extern BtechScriptFunction fun_btengrate_ref;
extern BtechScriptFunction fun_btfasabasecost_ref;
extern BtechScriptFunction fun_btgetbv;
extern BtechScriptFunction fun_btgetbv_ref;
extern BtechScriptFunction fun_btgetbv2_ref;
extern BtechScriptFunction fun_btgetcharvalue;
extern BtechScriptFunction fun_btgetdbv_ref;
extern BtechScriptFunction fun_btgetobv_ref;
extern BtechScriptFunction fun_btgetpartcost;
extern BtechScriptFunction fun_btgetrange;
extern BtechScriptFunction fun_btgetrealmaxspeed;
extern BtechScriptFunction fun_btgetweight;
extern BtechScriptFunction fun_btgetxcodevalue;
extern BtechScriptFunction fun_btgetxcodevalue_ref;
extern BtechScriptFunction fun_bthexemit;
extern BtechScriptFunction fun_bthexinblz;
extern BtechScriptFunction fun_bthexlos;
extern BtechScriptFunction fun_btid2db;
extern BtechScriptFunction fun_btlag;
extern BtechScriptFunction fun_btlistblz;
extern BtechScriptFunction fun_btloadmap;
extern BtechScriptFunction fun_btloadmech;
extern BtechScriptFunction fun_btlosm2m;
extern BtechScriptFunction fun_btmakepilotroll;
extern BtechScriptFunction fun_btmapelev;
extern BtechScriptFunction fun_btmapemit;
extern BtechScriptFunction fun_btmapterr;
extern BtechScriptFunction fun_btmapunits;
extern BtechScriptFunction fun_btmechfreqs;
extern BtechScriptFunction fun_btnumrepjobs;
extern BtechScriptFunction fun_btpartmatch;
extern BtechScriptFunction fun_btpartname;
extern BtechScriptFunction fun_btpartscategorylist;
extern BtechScriptFunction fun_btpartslist;
extern BtechScriptFunction fun_btparttype;
extern BtechScriptFunction fun_btpayload_ref;
extern BtechScriptFunction fun_btremovestores;
extern BtechScriptFunction fun_btsectstatus;
extern BtechScriptFunction fun_btsetarmorstatus;
extern BtechScriptFunction fun_btsetcharvalue;
extern BtechScriptFunction fun_btsetmaxspeed;
extern BtechScriptFunction fun_btsetpartcost;
extern BtechScriptFunction fun_btsettons;
extern BtechScriptFunction fun_btsetxcodevalue;
extern BtechScriptFunction fun_btsetxy;
extern BtechScriptFunction fun_btshowcritstatus_ref;
extern BtechScriptFunction fun_btshowstatus_ref;
extern BtechScriptFunction fun_btshowwspecs_ref;
extern BtechScriptFunction fun_btstores;
extern BtechScriptFunction fun_btstores_short;
extern BtechScriptFunction fun_bttechlist;
extern BtechScriptFunction fun_bttechlist_ref;
extern BtechScriptFunction fun_bttechstatus;
extern BtechScriptFunction fun_bttechtime;
extern BtechScriptFunction fun_btthreshold;
extern BtechScriptFunction fun_btticweaps;
extern BtechScriptFunction fun_btunderrepair;
extern BtechScriptFunction fun_btunitfixable;
extern BtechScriptFunction fun_btunitpartslist;
extern BtechScriptFunction fun_btunitpartslist_ref;
extern BtechScriptFunction fun_btupdatelinks;
extern BtechScriptFunction fun_btweapons;
extern BtechScriptFunction fun_btweaponstatus;
extern BtechScriptFunction fun_btweaponstatus_ref;
extern BtechScriptFunction fun_btweapstat;
extern BtechScriptFunction fun_zmechs;
