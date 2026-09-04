#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;

#include <stdbool.h>

#include "map_coordinates.h"
#include "mech_api_types.h"
#include "mux/support/alloc.h"
#include "section_types.h"

typedef struct Mech Mech;

typedef struct MechId {
  char text[3];
} MechId;

typedef struct WeaponArcRequest {
  Mech *mech;
  MapRealPosition target;
  int section;
  int critical;
} WeaponArcRequest;

/* mech.utils.c */
/* Misc Functions */
const char *mechtypename(Mech *foo);
int m_number(Mech *mech, int low, int high);
MechId mech_id(Mech *mech, bool lowercase);
char *my_to_upper(char *string);
void mark_for_los_update(Mech *mech);

int round_to_halfton(int weight);
int round_to_quarterton(int weight);

/* Self-inflicted kill types. (But flood might be accidental/intentional.) */
constexpr char KILL_TYPE_SELF_DESTRUCT[] = "SELF-DESTRUCT";
constexpr char KILL_TYPE_EJECT[] = "EJECT";
constexpr char KILL_TYPE_FLOOD[] = "FLOOD"; /* includes vacuum */
/* Accidental kill types. (But ice/heat might be intentional.) */
constexpr char KILL_TYPE_ICE[] = "FLOOD-ICE";
constexpr char KILL_TYPE_HEAT[] = "HEAT";
/* Intentional kill types.  */
constexpr char KILL_TYPE_NORMAL[] =
    "DESTROYED"; /* all other kills; includes carrier destruction */
constexpr char KILL_TYPE_PILOT[] =
    "PILOT"; /* Only happens on vehicles. Mainly crew death */
constexpr char KILL_TYPE_MWDAMAGE[] =
    "MWDAMAGE"; /* Failed MW Conc rolls once too many or one too many head hit
                 */
constexpr char KILL_TYPE_BEHEADED[] = "BEHEADED";
constexpr char KILL_TYPE_XLENGINE[] = "XLENGINE";
constexpr char KILL_TYPE_FUELTANK[] = "FUELTANK"; /* Fuel Tank Crit Death */
constexpr char KILL_TYPE_COCKPIT[] =
    "COCKPIT"; /* Alot different than Pilot death. Cockpit Crit death */
constexpr char KILL_TYPE_POWERPLANT[] =
    "POWERPLANT"; /* Vehicle powerplant crit death */
constexpr char KILL_TYPE_SCHARGE[] = "SCHARGE"; /* Super Charger overload */
constexpr char KILL_TYPE_TRANSPORT[] =
    "TRANSPORT"; /* The Transport containing the unit died */
constexpr char KILL_TYPE_ENGINE[] =
    "ENGINE"; /* Unit engined, standard fusion death */
constexpr char KILL_TYPE_HEAD_TARGET[] =
    "HEAD-TARGET"; /* Head was taken off, using TARGET */

void channel_emit_kill(Mech *mech, Mech *attacker, const char *reason);

typedef struct MapValidationRequest {
  BtechContext *context;
  DbRef player;
  DbRef map;
} MapValidationRequest;

typedef struct PilotSkillRollRequest {
  Mech *mech;
  int modifier;
  bool succeed_when_fallen;
} PilotSkillRollRequest;

BattleMap *valid_map(const MapValidationRequest *request);
DbRef find_mech_on_map(BattleMap *map, const char *mechid);
Mech *find_mech_in_hex(Mech *mech, BattleMap *mech_map, int x, int y,
                       int needlos);
DbRef find_target_dbref_from_map_number(Mech *mech, const char *mapnum);

/* Map Math */
int acceptable_degree(int d);
MapRealPosition map_project_position(const MapProjection *projection);
float map_spatial_range(const MapSpatialSegment *segment);
typedef struct HexDistanceRequest {
  MapHexPosition start;
  MapHexPosition end;
  int correction;
} HexDistanceRequest;
int map_hex_distance(const HexDistanceRequest *request);
float map_real_range(const MapRealSegment *segment);
void real_coord_to_map_coord(short *hex_x, short *hex_y, float cart_x,
                             float cart_y);
void map_coord_to_real_coord(int hex_x, int hex_y, float *cart_x,
                             float *cart_y);
typedef void (*NeighborHexCallback)(BattleMap *map, int x, int y,
                                    void *context);
void visit_neighbor_hexes(BattleMap *map, int x, int y,
                          NeighborHexCallback callback, void *context);
MapRealPosition map_vector_components(const MapPolarVector *vector);
void check_edge_of_map(Mech *mech);
int map_vertical_bearing(const MapSpatialSegment *segment);
int map_bearing(const MapRealSegment *segment);
int in_weapon_arc(Mech *mech, float x, float y);
bool is_in_weapon_arc(const WeaponArcRequest *request);
typedef struct NavigatePlotCall {
  int row;
  int column;
  char marker;
  void *context;
} NavigatePlotCall;
typedef void (*NavigatePlotCallback)(const NavigatePlotCall *call);
typedef struct NavigateSketchRequest {
  Mech *mech;
  BattleMap *map;
  MapHexPosition center;
  NavigatePlotCallback plot;
  void *context;
} NavigateSketchRequest;
void navigate_sketch_mechs(const NavigateSketchRequest *request);
typedef struct MechTargetPositionResult {
  bool found;
  MapSpatialPosition position;
} MechTargetPositionResult;
MechTargetPositionResult mech_target_position(const Mech *mech);

/* Skill lookups */
const char *find_gunnery_skill_name(Mech *mech, int weapindx);
const char *find_piloting_skill_name(Mech *mech);
int find_pilot_piloting(Mech *mech);
int find_s_pilot_piloting(Mech *mech);
int find_pilot_spotting(Mech *mech);
int find_pilot_arty_gun(Mech *mech);
int find_pilot_gunnery(Mech *mech, int weapindx);
const char *find_tech_skill_name(Mech *mech);
int find_tech_skill(DbRef player, Mech *mech);

/* Skill rolls */
long btech_random_range(BtechContext *context, long low, long high);
int btech_random_range_int(BtechContext *context, int low, int high);
bool made_pilot_skill_roll(Mech *mech, int mods);
int mech_pilot_skill_roll_target(Mech *mech, int mods);
bool mech_pilot_skill_roll(const PilotSkillRollRequest *request);
bool mech_pilot_skill_roll_without_experience(
    const PilotSkillRollRequest *request);
int btech_random_roll(BtechContext *context);

/* Section/Crit Functions */
int crits_in_loc(Mech *mech, int index);
bool sect_has_busy_weap(Mech *mech, int sect);
int find_weapons_advanced(Mech *mech, int index, unsigned char *weaparray,
                          unsigned char *weapdataarray, int *critical,
                          int whine);
int find_ammunition(Mech *mech, unsigned char *weaparray,
                    unsigned short *ammoarray, unsigned short *ammomaxarray,
                    unsigned int *modearray, int returnall);
int find_leg_heat_sinks(Mech *mech);
typedef struct WeaponNumberLookupRequest {
  Mech *mech;
  int number;
  bool sight;
} WeaponNumberLookupRequest;

typedef struct WeaponNumberLookupResult {
  bool found;
  int value;
  CriticalSlotReference slot;
} WeaponNumberLookupResult;

WeaponNumberLookupResult
weapon_number_find(const WeaponNumberLookupRequest *request);
int find_weapon_index(Mech *mech, int number);
int find_ammo_in_section(Mech *mech, int section, int type, int nogof, int gof);
int full_ammo(const Mech *mech, int loc, int pos);
typedef struct AmmunitionLookupRequest {
  Mech *mech;
  CriticalSlotReference weapon;
  bool use_weapon_preference;
  int weapon_index;
  int start_section;
  int forbidden_modes;
  int required_modes;
} AmmunitionLookupRequest;

CriticalSlotLookupResult
ammunition_find(const AmmunitionLookupRequest *request);
int count_ammo_for_weapon(Mech *mech, int weapindx);
bool find_artemis_for_weapon(Mech *mech, int section, int critical);
int reverse_split_crit_loc(Mech *mech, int sect, int crit);
int find_split_crits(Mech *mech, int sect, int type, int crit);
typedef struct SplitCriticalLookup {
  bool found;
  CriticalSlotReference slot;
  int part_type;
} SplitCriticalLookup;

SplitCriticalLookup split_critical_find(Mech *mech,
                                        CriticalSlotReference source);

typedef struct AmmunitionHazardResult {
  int damage;
  CriticalSlotReference slot;
} AmmunitionHazardResult;

AmmunitionHazardResult destructive_ammunition_find(Mech *mech);
AmmunitionHazardResult inferno_ammunition_find(Mech *mech);
int find_rounds_for_weapon(Mech *mech, int weapindx);
int heat_factor(Mech *mech);
int weapon_is_nonfunctional(Mech *mech, int section, int crit, int numcrits);
const char *const *proper_section_string_from_type(int type, int mtype);
typedef struct UnitSectionCatalog {
  int unit_type;
  int movement_type;
} UnitSectionCatalog;

size_t unit_section_name_count(const UnitSectionCatalog *catalog);
const char *unit_section_name(const UnitSectionCatalog *catalog, size_t index);

/* Minimum storage a caller must provide to armor_string_from_index. The
 * longest section name is "Front Right Side" (16 characters), and the
 * out-of-range marker "Invalid!!" is shorter, so 24 bytes leaves headroom
 * for new names without revisiting every caller. */
constexpr size_t UNIT_SECTION_NAME_CAPACITY = 24;

void armor_string_from_index(int index,
                             char buffer[static UNIT_SECTION_NAME_CAPACITY],
                             UnitClass type, MechMovementType movement_type);
int get_weapon_crits(Mech *mech, int weapindx);
int listmatch(const char *const *values, size_t value_count, const char *match);
typedef struct MultiWeaponSelectionCall {
  Mech *mech;
  DbRef actor;
  int first;
  int last;
  void *context;
} MultiWeaponSelectionCall;
typedef bool (*MultiWeaponSelectionCallback)(
    const MultiWeaponSelectionCall *call);
typedef struct MultiWeaponSelectionRequest {
  Mech *mech;
  DbRef actor;
  char *selection;
  int mode;
  MultiWeaponSelectionCallback callback;
  void *context;
} MultiWeaponSelectionRequest;
void multi_weapon_select(const MultiWeaponSelectionRequest *request);

/* Tech/Repair functions */
void do_sub_magic(Mech *mech, int loud);
void do_magic(Mech *mech);
void do_fixextra(Mech *mech);
void mech_repair_part(Mech *mech, int loc, int pos);
bool no_locations_destroyed(Mech *mech);
void mech_re_attach(Mech *mech, int loc);
void mech_replace_suit(Mech *mech, int loc);
void mech_re_seal(Mech *mech, int loc);
void mech_detach(Mech *mech, int loc);
void mech_fill_part_ammo(Mech *mech, int loc, int pos);

int count_destroyed_legs(Mech *obj_mech);
int is_leg_destroyed(Mech *obj_mech, int w_loc);
bool is_mech_leg_less(Mech *obj_mech);
typedef struct WeaponCriticalSearch {
  Mech *mech;
  CriticalSlotReference weapon;
  int start_critical;
  int part_type;
  int maximum_criticals;
} WeaponCriticalSearch;
int mech_weapon_first_critical(const WeaponCriticalSearch *search);
bool check_all_sections(Mech *mech, int special_to_find);
bool check_section_for_special(Mech *mech, int special_to_find, int w_sec);
int get_remaining_internal_percent(Mech *mech);
int get_remaining_armor_percent(Mech *mech);
int find_obj(Mech *mech, int loc, int type);
int find_obj_with_dest(Mech *mech, int loc, int type);
typedef struct AmmunitionCheckRequest {
  Mech *mech;
  int weapon_index;
  CriticalSlotReference weapon;
  int gatling_shots;
} AmmunitionCheckRequest;

typedef struct AmmunitionCheckResult {
  bool available;
  CriticalSlotLookupResult primary;
  CriticalSlotLookupResult secondary;
  int gatling_shots;
} AmmunitionCheckResult;

AmmunitionCheckResult ammunition_check(const AmmunitionCheckRequest *request);

int mech_armorpoints(Mech *mech);
int mech_intpoints(Mech *mech);
void unit_parts_list(Mech *mech, char buffer[static LBUF_SIZE]);
int mech_recycling_state(Mech *mech, int num);
