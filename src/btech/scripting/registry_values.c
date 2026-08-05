#include "values_internal.h"

[[maybe_unused]] static Mech tmpm;
[[maybe_unused]] static BattleMap tmpmap;
/* INDENT OFF */
const int scode_in_out[TYPE_LAST_TYPE] =
    /* st ch sh in fl db sf sfb sfs bv sfbd cbv ro-ch ro-sh ro-in ro-fl ro-db*/
    {3, 3, 3, 3, 3, 3, 1, 1, 2, 3, 3, 3, 3, 1, 1, 1, 1, 1};
/* INDENT ON */

#undef offsetof
#ifdef __compiler_offsetof
#define offsetof(TYPE, MEMBER) (void *)__compiler_offsetof(TYPE, MEMBER)
#else
#define offsetof(TYPE, MEMBER) (void *)((size_t)&((TYPE *)0)->MEMBER)
#endif

GMV xcode_data[] = {
    {GTYPE_MECH, "mapindex", offsetof(Mech, mapindex), TYPE_DBREF_RO, 0},
    {GTYPE_MECH, "id", mechIDfunc, TYPE_STRFUNC_BUF, 0},
    MeEntryS("mechname", MechType_Name, TYPE_STRING, 31),
    MeEntry("maxspeed", MechMaxSpeed, TYPE_FLOAT),
    MeEntryS("unit_era", MechUnitEra, TYPE_STRING, 25),
    MeEntryS("unit_tro", MechUnitTRO, TYPE_STRING, 25),
    MeEntry("templatesp", TemplateMaxSpeed, TYPE_FLOAT),
    MeEntry("pilotnum", MechPilot, TYPE_DBREF),
    MeEntry("xpmod", MechXPMod, TYPE_FLOAT),
    MeEntry("pilotdam", MechPilotStatus, TYPE_CHAR),
    MeEntry("si", AeroSI, TYPE_CHAR),
    MeEntry("si_orig", AeroSIOrig, TYPE_CHAR),
    MeEntry("speed", MechSpeed, TYPE_FLOAT),
    MeEntry("basewalkspeed", MechBaseWalk, TYPE_INT),
    MeEntry("baserunspeed", MechBaseRun, TYPE_INT),
    MeEntry("heading", MechRFacing, TYPE_SHORT),
    MeEntry("stall", MechStall, TYPE_INT),
    MeEntry("status", MechStatus, TYPE_BV),
    MeEntry("status2", MechStatus2, TYPE_BV),
    MeEntry("critstatus", MechCritStatus, TYPE_BV),
    MeEntry("critstatus2", MechCritStatus2, TYPE_BV),
    MeEntry("tankcritstatus", MechTankCritStatus, TYPE_BV),
    MeEntry("target", MechTarget, TYPE_DBREF),
    MeEntry("team", MechTeam, TYPE_INT),
    MeEntry("tons", MechTons, TYPE_INT),
    MeEntry("towing", MechCarrying, TYPE_INT_RO),
    MeEntry("heat", MechPlusHeat, TYPE_FLOAT),
    MeEntry("disabled_hs", MechDisabledHS, TYPE_INT_RO),
    MeEntry("overheat", MechHeat, TYPE_FLOAT),
    MeEntry("dissheat", MechMinusHeat, TYPE_FLOAT),
    MeEntry("hsengoverride", MechHSEngOverRide, TYPE_INT),
    MeEntry("heatsinks", MechRealNumsinks, TYPE_CHAR_RO),
    MeEntry("last_startup", MechLastStartup, TYPE_INT),
    MeEntry("C3iNetworkSize", MechC3iNetworkSize, TYPE_INT_RO),
    MeEntry("MaxSuits", MechMaxSuits, TYPE_INT),
    MeEntry("realweight", MechRTonsV, TYPE_INT),
    MeEntry("StaggerDamage", StaggerDamage, TYPE_INT_RO),
    MeEntry("MechPrefs", MechPrefs, TYPE_BV),
    MeEntry("SwarmTarget", MechSwarmTarget, TYPE_DBREF),
    MeEntry("SwarmedBy", MechSwarmer, TYPE_DBREF),

    {GTYPE_MECH, "mechtype", mechTypefunc, TYPE_STRFUNC_BD, 0},
    {GTYPE_MECH, "mechmovetype", mechMovefunc, TYPE_STRFUNC_BD, 0},
    {GTYPE_MECH, "mechdamage", mechDamagefunc, TYPE_STRFUNC_BD_BUF, 0},
    {GTYPE_MECH, "techtime", mechTechTimefunc, TYPE_STRFUNC_BUF, 0},
    {GTYPE_MECH, "centdist", mechCentDistfunc, TYPE_STRFUNC_BUF, 0},
    {GTYPE_MECH, "centbearing", mechCentBearingfunc, TYPE_STRFUNC_BUF, 0},
    {GTYPE_MECH, "sensors", mech_sensor_info, TYPE_STRFUNC_BUF, 0},
    {GTYPE_MECH, "mechref", mech_getset_ref, TYPE_STRFUNC_BD, 0},

    MeEntry("fuel", AeroFuel, TYPE_INT),
    MeEntry("fuel_orig", AeroFuelOrig, TYPE_INT),
    MeEntry("cocoon", MechCocoon, TYPE_INT_RO),
    MeEntry("numseen", MechNumSeen, TYPE_SHORT),

    MeEntry("fx", MechFX, TYPE_FLOAT),
    MeEntry("fy", MechFY, TYPE_FLOAT),
    MeEntry("fz", MechFZ, TYPE_FLOAT),
    MeEntry("x", MechX, TYPE_SHORT),
    MeEntry("y", MechY, TYPE_SHORT),
    MeEntry("z", MechZ, TYPE_SHORT),
    MeEntry("elevation", MechElev, TYPE_CHAR),

    MeEntry("targcomp", MechTargComp, TYPE_CHAR),
    MeEntry("lrsrange", MechLRSRange, TYPE_CHAR),
    MeEntry("radiorange", MechRadioRange, TYPE_SHORT),
    MeEntry("scanrange", MechScanRange, TYPE_CHAR),
    MeEntry("tacrange", MechTacRange, TYPE_CHAR),
    MeEntry("radiotype", MechRadioType, TYPE_CHAR),
    MeEntry("bv", MechBV, TYPE_INT),
    MeEntry("cargospace", CargoSpace, TYPE_INT),
    MeEntry("carmaxton", CarMaxTon, TYPE_CHAR_RO),

    MeVEntry("bay0", AeroBay, 0, TYPE_DBREF),
    MeVEntry("bay1", AeroBay, 1, TYPE_DBREF),
    MeVEntry("bay2", AeroBay, 2, TYPE_DBREF),
    MeVEntry("bay3", AeroBay, 3, TYPE_DBREF),

    MeVEntry("turret0", AeroTurret, 0, TYPE_DBREF),
    MeVEntry("turret1", AeroTurret, 1, TYPE_DBREF),
    MeVEntry("turret2", AeroTurret, 2, TYPE_DBREF),

    MeEntry("unusablearcs", AeroUnusableArcs, TYPE_INT_RO),
    MeEntry("maxjumpspeed", MechJumpSpeed, TYPE_FLOAT),
    MeEntry("jumpheading", MechJumpHeading, TYPE_SHORT),
    MeEntry("jumplength", MechJumpLength, TYPE_SHORT),

    MaEntry("buildflag", buildflag, TYPE_CHAR),
    MaEntry("buildonmap", onmap, TYPE_DBREF_RO),
    MaEntry("cf", cf, TYPE_SHORT),
    MaEntry("cfmax", cfmax, TYPE_SHORT),
    MaEntry("gravity", grav, TYPE_CHAR),
    MaEntry("firstfree", first_free, TYPE_CHAR_RO),
    MaEntry("mapheight", map_height, TYPE_SHORT_RO),
    MaEntry("maplight", maplight, TYPE_CHAR),
    MaEntryS("mapname", mapname, TYPE_STRING, 30),
    MaEntry("mapvis", mapvis, TYPE_CHAR),
    MaEntry("mapwidth", map_width, TYPE_SHORT_RO),
    MaEntry("maxvis", maxvis, TYPE_SHORT),
    MaEntry("temperature", temp, TYPE_CHAR),
    MaEntry("winddir", winddir, TYPE_SHORT),
    MaEntry("windspeed", windspeed, TYPE_SHORT),
    MaEntry("cloudbase", cloudbase, TYPE_SHORT),
    MaEntry("flags", flags, TYPE_BV),
    MaEntry("sensorflags", sensorflags, TYPE_BV),
    MaEntry("regen_factor", regen_factor, TYPE_INT),

    TuEntry("arcs", arcs, TYPE_INT),
    TuEntry("parent", parent, TYPE_DBREF),
    TuEntry("gunner", gunner, TYPE_DBREF),
    TuEntry("target", target, TYPE_DBREF),
    TuEntry("targx", target, TYPE_SHORT),
    TuEntry("targy", target, TYPE_SHORT),
    TuEntry("targz", target, TYPE_SHORT),
    TuEntry("lockmode", lockmode, TYPE_INT),

    MeEntry("radio", MechRadio, TYPE_CHAR),
    MeEntry("computer", MechComputer, TYPE_CHAR),
    MeEntry("perception", MechPer, TYPE_INT),

    MeEntry("shots_fired", MechShotsFired, TYPE_INT),
    MeEntry("shots_missed", MechShotsMissed, TYPE_INT),
    MeEntry("shots_hit", MechShotsHit, TYPE_INT),
    MeEntry("damage_taken", MechDamageTaken, TYPE_INT),
    MeEntry("damage_inflicted", MechDamageInflicted, TYPE_INT),
    MeEntry("units_killed", MechUnitsKilled, TYPE_INT),
    MeEntry("hexes_walked", MechHexes, TYPE_FLOAT),

    {-1, NULL, 0, TYPE_STRING, 0}};

void fun_zmechs(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  DbRef it = match_thing(&context->command->match, player, fargs[0]);
  DbRef i;
  int len = 0;
  char reference[SBUF_SIZE];

  if (!is_controls(context->world->database, player, it) &&
      !is_wizard(context->btech->database, player)) {
    safe_str("#-1 NO PERMISSION TO USE", buff, bufc);
    return;
  }
  for (i = 0; i < context->btech->database->top; i++)
    if (typeof_obj(context->btech->database, i) == OBJECT_TYPE_THING) {
      if (game_object_zone(context->btech->database, i) == it) {
        if ((btech_context_which_special(context->btech, i) == GTYPE_MECH) &&
            is_good_obj(context->btech->database, i)) {
          if (len) {
            snprintf(reference, sizeof(reference), " #%ld", i);
            if ((strlen(reference) + len) > (LBUF_SIZE - SBUF_SIZE)) {
              safe_str(" #-1", buff, bufc);
              return;
            }
            safe_str(reference, buff, bufc);
            len += strlen(reference);
          } else {
            safe_tprintf_str(buff, bufc, "#%ld", i);
            len = strlen(buff);
          }
        }
      }
    }
}

void fun_btsetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = name of the value
     fargs[2] = what the value's to be set as
   */
  DbRef it;
  int i, spec;
  void *foo;
  void *bar;
  void *(*tempfun)(int, Mech *, char *);
  void *(*buffered_tempfun)(int, Mech *, char *, char *);

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  spec = btech_context_which_special(context->btech, it);
  FUNCHECK(!(foo = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 2)) {
      bar = (void *)((long)foo + xcode_data[i].rel_addr);
      switch (xcode_data[i].type) {
      case TYPE_STRFUNC_BD:
      case TYPE_STRFUNC_S:
        tempfun = (void *)xcode_data[i].rel_addr;
        tempfun(1, (Mech *)foo, (char *)fargs[2]);
        break;
      case TYPE_STRFUNC_BD_BUF:
        buffered_tempfun = (void *)xcode_data[i].rel_addr;
        buffered_tempfun(1, (Mech *)foo, (char *)fargs[2],
                         (char[LBUF_SIZE]){0});
        break;
      case TYPE_STRING:
        strncpy((char *)bar, fargs[2], xcode_data[i].size - 1);
        ((char *)bar)[xcode_data[i].size - 1] = '\0';
        break;
      case TYPE_DBREF:
        *((DbRef *)bar) = atoi(fargs[2]);
        break;
      case TYPE_CHAR:
        *((char *)bar) = atoi(fargs[2]);
        break;
      case TYPE_SHORT:
        *((short *)bar) = atoi(fargs[2]);
        break;
      case TYPE_INT:
        *((int *)bar) = atoi(fargs[2]);
        break;
      case TYPE_FLOAT:
        *((float *)bar) = atof(fargs[2]);
        break;
      case TYPE_BV:
        *((int *)bar) = text2bv(fargs[2]);
        break;
      case TYPE_CBV:
        *((byte *)bar) = (byte)text2bv(fargs[2]);
        break;
      }
      safe_tprintf_str(buff, bufc, "1");
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

static char *retrieve_value(void *data, int i, char *buffer) {
  void *bar = (void *)((long)data + xcode_data[i].rel_addr);
  char *(*tempfun)(int, Mech *);
  char *(*buffered_tempfun)(Mech *, char *);
  char *(*buffered_bidirectional_tempfun)(int, Mech *, char *, char *);

  switch (xcode_data[i].type) {
  case TYPE_STRFUNC_BD:
  case TYPE_STRFUNC:
    tempfun = (void *)xcode_data[i].rel_addr;
    snprintf(buffer, LBUF_SIZE, "%s", (char *)tempfun(0, (Mech *)data));
    break;
  case TYPE_STRFUNC_BUF:
    buffered_tempfun = (void *)xcode_data[i].rel_addr;
    buffered_tempfun((Mech *)data, buffer);
    break;
  case TYPE_STRFUNC_BD_BUF:
    buffered_bidirectional_tempfun = (void *)xcode_data[i].rel_addr;
    buffered_bidirectional_tempfun(0, (Mech *)data, nullptr, buffer);
    break;
  case TYPE_STRING:
    snprintf(buffer, LBUF_SIZE, "%s", (char *)bar);
    break;
  case TYPE_DBREF:
  case TYPE_DBREF_RO:
    snprintf(buffer, LBUF_SIZE, "%ld", (DbRef) * ((DbRef *)bar));
    break;
  case TYPE_CHAR:
  case TYPE_CHAR_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (char)*((char *)bar));
    break;
  case TYPE_SHORT:
  case TYPE_SHORT_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (short)*((short *)bar));
    break;
  case TYPE_INT:
  case TYPE_INT_RO:
    snprintf(buffer, LBUF_SIZE, "%d", (int)*((int *)bar));
    break;
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO:
    snprintf(buffer, LBUF_SIZE, "%.2f", (float)*((float *)bar));
    break;
  case TYPE_BV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text((int)*((int *)bar), (char[SBUF_SIZE]){0}));
    break;
  case TYPE_CBV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text((int)*((char *)bar), (char[SBUF_SIZE]){0}));
    break;
  }
  return buffer;
}

void fun_btgetxcodevalue(char *buff, char **bufc, DbRef player, DbRef cause,
                         char *fargs[], int nfargs, char *cargs[], int ncargs,
                         EvaluationContext *context) {
  /* fargs[0] = id of the mech
     fargs[1] = name of the value
   */
  DbRef it;
  int i;
  void *foo;
  int spec;

  it = match_thing(&context->command->match, player, fargs[0]);
  FUNCHECK(it == NOTHING ||
               !is_examinable(context->world->database, player, it),
           "#-1");
  spec = btech_context_which_special(context->btech, it);
  FUNCHECK(!(foo = btech_context_find_object(context->btech, it)), "#-1");
  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 1)) {
      safe_tprintf_str(buff, bufc, "%s",
                       retrieve_value(foo, i, (char[LBUF_SIZE]){0}));
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void fun_btgetxcodevalue_ref(char *buff, char **bufc, DbRef player, DbRef cause,
                             char *fargs[], int nfargs, char *cargs[],
                             int ncargs, EvaluationContext *context) {
  /* fargs[0] = mech ref
     fargs[1] = name of the value
   */
  int i;
  Mech *foo;
  int spec;

  FUNCHECK(!is_wizard(context->world->database, player),
           "#-1 PERMISSION DENIED");
  FUNCHECK((foo = load_refmech(context->btech, fargs[0])) == NULL,
           "#-1 NO SUCH MECH");
  spec = GTYPE_MECH;
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(fargs[1], xcode_data[i].name) &&
        xcode_data[i].gtype == spec && (scode_in_out[xcode_data[i].type] & 1)) {
      safe_tprintf_str(buff, bufc, "%s",
                       retrieve_value(foo, i, (char[LBUF_SIZE]){0}));
      return;
    }
  safe_tprintf_str(buff, bufc, "#-1");
  return;
}

void set_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  char *args[2];
  int t, i;
  void *bar;
  void *(*tempfun)(int, Mech *, char *);
  void *(*buffered_tempfun)(int, Mech *, char *, char *);

  memset(args, 0, sizeof(char *) * 2);

  DOCHECK_CONTEXT(context, silly_parseattributes(buffer, args, 2) != 2,
                  "Invalid arguments!");
  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  for (i = 0; xcode_data[i].name; i++)
    if (xcode_data[i].gtype == t)
      break;
  DOCHECK_CONTEXT(context, !xcode_data[i].name,
                  "Error: No xcode values for this type of object found.");
  for (i = 0; xcode_data[i].name; i++)
    if (!strcasecmp(args[0], xcode_data[i].name) && xcode_data[i].gtype == t &&
        (scode_in_out[xcode_data[i].type] & 2))
      break;
  DOCHECK_CONTEXT(
      context, !xcode_data[i].name,
      "Error: No matching xcode value for this type of object found.");
  bar = (void *)((long)btech_context_find_object(
                     context, game_object_location(context->database, player)) +
                 xcode_data[i].rel_addr);
  switch (xcode_data[i].type) {
  case TYPE_STRFUNC_BD:
  case TYPE_STRFUNC_S:
    tempfun = (void *)xcode_data[i].rel_addr;
    tempfun(1,
            btech_context_get_mech(
                context, game_object_location(context->database, player)),
            (char *)args[1]);
    break;
  case TYPE_STRFUNC_BD_BUF:
    buffered_tempfun = (void *)xcode_data[i].rel_addr;
    buffered_tempfun(
        1,
        btech_context_get_mech(context,
                               game_object_location(context->database, player)),
        (char *)args[1], (char[LBUF_SIZE]){0});
    break;
  case TYPE_STRING:
    strncpy((char *)bar, args[1], xcode_data[i].size - 1);
    ((char *)bar)[xcode_data[i].size - 1] = '\0';
    break;
  case TYPE_DBREF:
    *((DbRef *)bar) = atoi(args[1]);
    break;
  case TYPE_CHAR:
    *((char *)bar) = atoi(args[1]);
    break;
  case TYPE_SHORT:
    *((short *)bar) = atoi(args[1]);
    break;
  case TYPE_INT:
    *((int *)bar) = atoi(args[1]);
    break;
  case TYPE_FLOAT:
    *((float *)bar) = atof(args[1]);
    break;
  case TYPE_BV:
    *((int *)bar) = text2bv(args[1]);
    break;
  case TYPE_CBV:
    *((byte *)bar) = (byte)text2bv(args[1]);
  }
}

void list_xcodestuff(DbRef player, void *data, char *buffer) {
  BtechContext *context = ((BtechSpecialObject *)data)->context;
  int t, i, flag = CM_TWO, se_len = 37;
  CoolMenu *c = NULL;

  t = btech_context_which_special(
      context, game_object_location(context->database, player));
  for (i = 0; xcode_data[i].name; i++)
    if (xcode_data[i].gtype == t && (scode_in_out[xcode_data[i].type] & 1))
      break;
  DOCHECK_CONTEXT(context, !xcode_data[i].name,
                  "Error: No xcode values for this type of object found.");
  addline();
  cent(
      tprintf("Data for %s (%s)",
              game_object_name(context->database,
                               game_object_location(context->database, player)),
              SpecialObjects[t].type));
  addline();
  if (*buffer == '1') {
    flag = CM_ONE;
    se_len = se_len * 2;
  };
  if (*buffer == '4') {
    flag = CM_FOUR;
    se_len = se_len / 2;
  };
  if (*buffer == '1' || *buffer == '4')
    buffer++;
  for (i = 0; xcode_data[i].name; i++) {
    if (xcode_data[i].gtype == t && (scode_in_out[xcode_data[i].type] & 1)) {
      /* 1/3(left) = name, 2/3(right)=value */
      char mask[SBUF_SIZE];
      char lab[SBUF_SIZE];

      if (*buffer)
        if (strncasecmp(xcode_data[i].name, buffer, strlen(buffer)))
          continue;
      strcpy(lab, xcode_data[i].name);
      lab[se_len / 3] = 0;
      snprintf(mask, SBUF_SIZE, "%%-%ds%%%ds", se_len / 3, se_len * 2 / 3);
      /* mask is built above from a fixed pattern, not external input. */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
#endif
      sim(tprintf(mask, lab, retrieve_value(data, i, (char[LBUF_SIZE]){0})),
          flag);
#ifdef __clang__
#pragma clang diagnostic pop
#endif
    }
  }
  addline();
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}
