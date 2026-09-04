#pragma once

#include <stddef.h>

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

typedef struct BtechScriptArguments {
  char **values;
  size_t count;
} BtechScriptArguments;

typedef struct BtechScriptOutput {
  char *buffer;
  char *cursor;
  size_t capacity;
} BtechScriptOutput;

typedef struct BtechScriptCall {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef cause;
  BtechScriptOutput output;
  BtechScriptArguments arguments;
  BtechScriptArguments command_arguments;
} BtechScriptCall;

typedef enum BtechScriptStatus : int {
  BTECH_SCRIPT_OK,
  BTECH_SCRIPT_ERROR,
} BtechScriptStatus;

typedef enum BtechScriptValueKind : int {
  BTECH_SCRIPT_TEXT,
  BTECH_SCRIPT_LIST,
  BTECH_SCRIPT_NUMBER,
  BTECH_SCRIPT_BOOLEAN,
  BTECH_SCRIPT_MUTATION,
} BtechScriptValueKind;

typedef enum BtechScriptListItemKind : int {
  BTECH_SCRIPT_LIST_TEXT,
  BTECH_SCRIPT_LIST_NUMBER,
} BtechScriptListItemKind;

typedef struct BtechScriptListItem {
  BtechScriptListItemKind kind;
  union {
    const char *text;
    long number;
  } value;
} BtechScriptListItem;

typedef struct BtechScriptList {
  BtechScriptListItem *items;
  size_t count;
} BtechScriptList;

typedef struct BtechScriptResult {
  BtechScriptStatus status;
  BtechScriptValueKind kind;
  union {
    const char *text;
    BtechScriptList list;
    double number;
    bool boolean;
    bool mutation;
  } value;
} BtechScriptResult;

BtechScriptResult btech_script_result_finish(BtechScriptCall *call,
                                             BtechScriptValueKind kind);
[[nodiscard]] BtechScriptResult btech_script_error(BtechScriptCall *call,
                                                   const char *message);
[[nodiscard]] BtechScriptResult
btech_script_error_output(BtechScriptCall *call);
void btech_script_result_destroy(BtechScriptResult *result);

typedef BtechScriptResult BtechScriptFunction(BtechScriptCall *call);

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
extern BtechScriptFunction fun_btgetunitvalue;
extern BtechScriptFunction fun_btgetunitvalue_ref;
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
extern BtechScriptFunction fun_btsetunitvalue;
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
extern BtechScriptFunction fun_btunitdisplayname;
extern BtechScriptFunction fun_btsetunitdisplayname;
extern BtechScriptFunction fun_btunitfixable;
extern BtechScriptFunction fun_btunitpartslist;
extern BtechScriptFunction fun_btunitpartslist_ref;
extern BtechScriptFunction fun_btupdatelinks;
extern BtechScriptFunction fun_btweaponstatus;
extern BtechScriptFunction fun_btweaponstatus_ref;
extern BtechScriptFunction fun_btweapstat;
extern BtechScriptFunction fun_zmechs;
