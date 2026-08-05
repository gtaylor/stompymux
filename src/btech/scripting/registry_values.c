#include "values_internal.h"

[[maybe_unused]] static BattleMap tmpmap;
/* INDENT OFF */
const int scode_in_out[TYPE_LAST_TYPE] =
    /* st ch sh in fl db sf sfb sfs bv sfbd cbv ro-ch ro-sh ro-in ro-fl ro-db*/
    {3, 3, 3, 3, 3, 3, 1, 1, 2, 3, 3, 3, 3, 1, 1, 1, 1, 1};
/* INDENT ON */

GMV xcode_data[] = {
    MeField("mapindex", MECH_SCRIPT_MAP_DBREF, TYPE_DBREF_RO),
    MeFunction("id", mechIDfunc, TYPE_STRFUNC_BUF),
    MeFieldS("mechname", MECH_SCRIPT_NAME, TYPE_STRING, 31),
    MeField("maxspeed", MECH_SCRIPT_MAXIMUM_SPEED, TYPE_FLOAT),
    MeFieldS("unit_era", MECH_SCRIPT_ERA, TYPE_STRING, 25),
    MeFieldS("unit_tro", MECH_SCRIPT_TRO, TYPE_STRING, 25),
    MeField("templatesp", MECH_SCRIPT_TEMPLATE_SPEED, TYPE_FLOAT),
    MeField("pilotnum", MECH_SCRIPT_PILOT_DBREF, TYPE_DBREF),
    MeField("xpmod", MECH_SCRIPT_EXPERIENCE_MODIFIER, TYPE_FLOAT),
    MeField("pilotdam", MECH_SCRIPT_PILOT_DAMAGE, TYPE_CHAR),
    MeField("si", MECH_SCRIPT_STRUCTURAL_INTEGRITY, TYPE_CHAR),
    MeField("si_orig", MECH_SCRIPT_ORIGINAL_STRUCTURAL_INTEGRITY, TYPE_CHAR),
    MeField("speed", MECH_SCRIPT_SPEED, TYPE_FLOAT),
    MeField("basewalkspeed", MECH_SCRIPT_BASE_WALK_SPEED, TYPE_INT),
    MeField("baserunspeed", MECH_SCRIPT_BASE_RUN_SPEED, TYPE_INT),
    MeField("heading", MECH_SCRIPT_HEADING, TYPE_SHORT),
    MeField("stall", MECH_SCRIPT_STALL, TYPE_INT),
    MeField("status", MECH_SCRIPT_STATUS, TYPE_BV),
    MeField("status2", MECH_SCRIPT_STATUS_SECONDARY, TYPE_BV),
    MeField("critstatus", MECH_SCRIPT_CRITICAL_STATUS, TYPE_BV),
    MeField("critstatus2", MECH_SCRIPT_CRITICAL_STATUS_SECONDARY, TYPE_BV),
    MeField("tankcritstatus", MECH_SCRIPT_VEHICLE_CRITICAL_STATUS, TYPE_BV),
    MeField("target", MECH_SCRIPT_TARGET_DBREF, TYPE_DBREF),
    MeField("team", MECH_SCRIPT_TEAM, TYPE_INT),
    MeField("tons", MECH_SCRIPT_TONNAGE, TYPE_INT),
    MeField("towing", MECH_SCRIPT_TOWING_DBREF, TYPE_INT_RO),
    MeField("heat", MECH_SCRIPT_HEAT_PRODUCTION, TYPE_FLOAT),
    MeField("disabled_hs", MECH_SCRIPT_DISABLED_HEAT_SINKS, TYPE_INT_RO),
    MeField("overheat", MECH_SCRIPT_HEAT, TYPE_FLOAT),
    MeField("dissheat", MECH_SCRIPT_HEAT_DISSIPATION, TYPE_FLOAT),
    MeField("hsengoverride", MECH_SCRIPT_ENGINE_HEAT_SINK_OVERRIDE, TYPE_INT),
    MeField("heatsinks", MECH_SCRIPT_HEAT_SINKS, TYPE_CHAR_RO),
    MeField("last_startup", MECH_SCRIPT_LAST_STARTUP, TYPE_INT),
    MeField("C3iNetworkSize", MECH_SCRIPT_C3I_NETWORK_SIZE, TYPE_INT_RO),
    MeField("MaxSuits", MECH_SCRIPT_MAXIMUM_BATTLE_SUITS, TYPE_INT),
    MeField("realweight", MECH_SCRIPT_REAL_WEIGHT, TYPE_INT),
    MeField("StaggerDamage", MECH_SCRIPT_STAGGER_DAMAGE, TYPE_INT_RO),
    MeField("MechPrefs", MECH_SCRIPT_PREFERENCES, TYPE_BV),
    MeField("SwarmTarget", MECH_SCRIPT_SWARM_TARGET, TYPE_DBREF),
    MeField("SwarmedBy", MECH_SCRIPT_SWARMER, TYPE_DBREF),

    MeFunction("mechtype", mechTypefunc, TYPE_STRFUNC_BD),
    MeFunction("mechmovetype", mechMovefunc, TYPE_STRFUNC_BD),
    MeFunction("mechdamage", mechDamagefunc, TYPE_STRFUNC_BD_BUF),
    MeFunction("techtime", mechTechTimefunc, TYPE_STRFUNC_BUF),
    MeFunction("centdist", mechCentDistfunc, TYPE_STRFUNC_BUF),
    MeFunction("centbearing", mechCentBearingfunc, TYPE_STRFUNC_BUF),
    MeFunction("sensors", mech_sensor_info, TYPE_STRFUNC_BUF),
    MeFunction("mechref", mech_getset_ref, TYPE_STRFUNC_BD),

    MeField("fuel", MECH_SCRIPT_FUEL, TYPE_INT),
    MeField("fuel_orig", MECH_SCRIPT_ORIGINAL_FUEL, TYPE_INT),
    MeField("cocoon", MECH_SCRIPT_COCOON, TYPE_INT_RO),
    MeField("numseen", MECH_SCRIPT_SEEN_COUNT, TYPE_SHORT),

    MeField("fx", MECH_SCRIPT_REAL_X, TYPE_FLOAT),
    MeField("fy", MECH_SCRIPT_REAL_Y, TYPE_FLOAT),
    MeField("fz", MECH_SCRIPT_REAL_Z, TYPE_FLOAT),
    MeField("x", MECH_SCRIPT_X, TYPE_SHORT),
    MeField("y", MECH_SCRIPT_Y, TYPE_SHORT),
    MeField("z", MECH_SCRIPT_Z, TYPE_SHORT),
    MeField("elevation", MECH_SCRIPT_ELEVATION, TYPE_CHAR),

    MeField("targcomp", MECH_SCRIPT_TARGETING_COMPUTER, TYPE_CHAR),
    MeField("lrsrange", MECH_SCRIPT_LONG_RANGE_SENSOR_RANGE, TYPE_CHAR),
    MeField("radiorange", MECH_SCRIPT_RADIO_RANGE, TYPE_SHORT),
    MeField("scanrange", MECH_SCRIPT_SCAN_RANGE, TYPE_CHAR),
    MeField("tacrange", MECH_SCRIPT_TACTICAL_SENSOR_RANGE, TYPE_CHAR),
    MeField("radiotype", MECH_SCRIPT_RADIO_TYPE, TYPE_CHAR),
    MeField("bv", MECH_SCRIPT_BATTLE_VALUE, TYPE_INT),
    MeField("cargospace", MECH_SCRIPT_CARGO_SPACE, TYPE_INT),
    MeField("carmaxton", MECH_SCRIPT_CARRIER_MAXIMUM_TONNAGE, TYPE_CHAR_RO),

    MeField("bay0", MECH_SCRIPT_BAY_0, TYPE_DBREF),
    MeField("bay1", MECH_SCRIPT_BAY_1, TYPE_DBREF),
    MeField("bay2", MECH_SCRIPT_BAY_2, TYPE_DBREF),
    MeField("bay3", MECH_SCRIPT_BAY_3, TYPE_DBREF),

    MeField("turret0", MECH_SCRIPT_TURRET_0, TYPE_DBREF),
    MeField("turret1", MECH_SCRIPT_TURRET_1, TYPE_DBREF),
    MeField("turret2", MECH_SCRIPT_TURRET_2, TYPE_DBREF),

    MeField("unusablearcs", MECH_SCRIPT_UNUSABLE_ARCS, TYPE_INT_RO),
    MeField("maxjumpspeed", MECH_SCRIPT_MAXIMUM_JUMP_SPEED, TYPE_FLOAT),
    MeField("jumpheading", MECH_SCRIPT_JUMP_HEADING, TYPE_SHORT),
    MeField("jumplength", MECH_SCRIPT_JUMP_LENGTH, TYPE_SHORT),

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

    MeField("radio", MECH_SCRIPT_RADIO, TYPE_CHAR),
    MeField("computer", MECH_SCRIPT_COMPUTER, TYPE_CHAR),
    MeField("perception", MECH_SCRIPT_PERCEPTION, TYPE_INT),

    MeField("shots_fired", MECH_SCRIPT_SHOTS_FIRED, TYPE_INT),
    MeField("shots_missed", MECH_SCRIPT_SHOTS_MISSED, TYPE_INT),
    MeField("shots_hit", MECH_SCRIPT_SHOTS_HIT, TYPE_INT),
    MeField("damage_taken", MECH_SCRIPT_DAMAGE_TAKEN, TYPE_INT),
    MeField("damage_inflicted", MECH_SCRIPT_DAMAGE_INFLICTED, TYPE_INT),
    MeField("units_killed", MECH_SCRIPT_UNITS_KILLED, TYPE_INT),
    MeField("hexes_walked", MECH_SCRIPT_HEXES_WALKED, TYPE_FLOAT),

    {-1, nullptr, nullptr, TYPE_STRING, 0, false, 0}};

static bool mech_value_write_text(Mech *mech, const GMV *descriptor,
                                  const char *text) {
  MechScriptValue value = {0};

  switch (descriptor->type) {
  case TYPE_STRING:
    value.string = text;
    break;
  case TYPE_DBREF:
    value.dbref = atoi(text);
    break;
  case TYPE_FLOAT:
    value.floating = atof(text);
    break;
  case TYPE_BV:
  case TYPE_CBV:
    value.integer = text2bv((char *)text);
    break;
  default:
    value.integer = atoi(text);
    break;
  }

  return mech_script_value_write(mech, descriptor->mech_key, value);
}

static char *mech_value_read_text(const Mech *mech, const GMV *descriptor,
                                  char *buffer) {
  MechScriptValue value = {0};
  if (!mech_script_value_read(mech, descriptor->mech_key, &value))
    return nullptr;

  switch (descriptor->type) {
  case TYPE_STRING:
    snprintf(buffer, LBUF_SIZE, "%s", value.string);
    break;
  case TYPE_DBREF:
  case TYPE_DBREF_RO:
    snprintf(buffer, LBUF_SIZE, "%ld", value.dbref);
    break;
  case TYPE_FLOAT:
  case TYPE_FLOAT_RO:
    snprintf(buffer, LBUF_SIZE, "%.2f", value.floating);
    break;
  case TYPE_BV:
  case TYPE_CBV:
    snprintf(buffer, LBUF_SIZE, "%s",
             bv2text(value.integer, (char[SBUF_SIZE]){0}));
    break;
  default:
    snprintf(buffer, LBUF_SIZE, "%d", value.integer);
    break;
  }
  return buffer;
}

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
      if (xcode_data[i].mech_value) {
        if (mech_value_write_text(foo, &xcode_data[i], fargs[2])) {
          safe_tprintf_str(buff, bufc, "1");
          return;
        }
        break;
      }
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
  if (xcode_data[i].mech_value)
    return mech_value_read_text(data, &xcode_data[i], buffer);

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
  if (xcode_data[i].mech_value) {
    DOCHECK_CONTEXT(
        context,
        !mech_value_write_text(
            btech_context_get_mech(
                context, game_object_location(context->database, player)),
            &xcode_data[i], args[1]),
        "Error: Unable to set that xcode value.");
    return;
  }
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
