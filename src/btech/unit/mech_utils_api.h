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
int MNumber(Mech *mech, int low, int high);
MechId mech_id(Mech *mech, bool lowercase);
char *MyToUpper(char *string);
void MarkForLOSUpdate(Mech *mech);

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
#define KILL_TYPE_NORMAL                                                       \
  "DESTROYED" /* all other kills; includes carrier destruction */
#define KILL_TYPE_PILOT                                                        \
  "PILOT" /* Only happens on vehicles. Mainly crew death */
#define KILL_TYPE_MWDAMAGE                                                     \
  "MWDAMAGE" /* Failed MW Conc rolls once too many or one too many head hit */
constexpr char KILL_TYPE_BEHEADED[] = "BEHEADED";
constexpr char KILL_TYPE_XLENGINE[] = "XLENGINE";
constexpr char KILL_TYPE_FUELTANK[] = "FUELTANK"; /* Fuel Tank Crit Death */
#define KILL_TYPE_COCKPIT                                                      \
  "COCKPIT" /* Alot different than Pilot death. Cockpit Crit death */
constexpr char KILL_TYPE_POWERPLANT[] =
    "POWERPLANT"; /* Vehicle powerplant crit death */
constexpr char KILL_TYPE_SCHARGE[] = "SCHARGE"; /* Super Charger overload */
#define KILL_TYPE_TRANSPORT                                                    \
  "TRANSPORT" /* The Transport containing the unit died */
constexpr char KILL_TYPE_ENGINE[] =
    "ENGINE"; /* Unit engined, standard fusion death */
#define KILL_TYPE_HEAD_TARGET                                                  \
  "HEAD-TARGET" /* Head was taken off, using TARGET */

void ChannelEmitKill(Mech *mech, Mech *attacker, const char *reason);

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
DbRef FindMechOnMap(BattleMap *map, const char *mechid);
Mech *find_mech_in_hex(Mech *mech, BattleMap *mech_map, int x, int y,
                       int needlos);
DbRef FindTargetDBREFFromMapNumber(Mech *mech, const char *mapnum);

/* Map Math */
int AcceptableDegree(int d);
MapRealPosition map_project_position(const MapProjection *projection);
float map_spatial_range(const MapSpatialSegment *segment);
typedef struct HexDistanceRequest {
  MapHexPosition start;
  MapHexPosition end;
  int correction;
} HexDistanceRequest;
int map_hex_distance(const HexDistanceRequest *request);
float map_real_range(const MapRealSegment *segment);
void RealCoordToMapCoord(short *hex_x, short *hex_y, float cart_x,
                         float cart_y);
void MapCoordToRealCoord(int hex_x, int hex_y, float *cart_x, float *cart_y);
typedef void (*NeighborHexCallback)(BattleMap *map, int x, int y,
                                    void *context);
void visit_neighbor_hexes(BattleMap *map, int x, int y,
                          NeighborHexCallback callback, void *context);
MapRealPosition map_vector_components(const MapPolarVector *vector);
void CheckEdgeOfMap(Mech *mech);
int map_vertical_bearing(const MapSpatialSegment *segment);
int map_bearing(const MapRealSegment *segment);
int InWeaponArc(Mech *mech, float x, float y);
bool IsInWeaponArc(const WeaponArcRequest *request);
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
const char *FindGunnerySkillName(Mech *mech, int weapindx);
const char *FindPilotingSkillName(Mech *mech);
int FindPilotPiloting(Mech *mech);
int FindSPilotPiloting(Mech *mech);
int FindPilotSpotting(Mech *mech);
int FindPilotArtyGun(Mech *mech);
int FindPilotGunnery(Mech *mech, int weapindx);
const char *FindTechSkillName(Mech *mech);
int FindTechSkill(DbRef player, Mech *mech);

/* Skill rolls */
long btech_random_range(BtechContext *context, long low, long high);
int btech_random_range_int(BtechContext *context, int low, int high);
int MadePilotSkillRoll(Mech *mech, int mods);
int mech_pilot_skill_roll_target(Mech *mech, int mods);
int mech_pilot_skill_roll(const PilotSkillRollRequest *request);
int mech_pilot_skill_roll_without_experience(
    const PilotSkillRollRequest *request);
int btech_random_roll(BtechContext *context);

/* Section/Crit Functions */
int CritsInLoc(Mech *mech, int index);
int SectHasBusyWeap(Mech *mech, int sect);
int FindWeapons_Advanced(Mech *mech, int index, unsigned char *weaparray,
                         unsigned char *weapdataarray, int *critical,
                         int whine);
int FindAmmunition(Mech *mech, unsigned char *weaparray,
                   unsigned short *ammoarray, unsigned short *ammomaxarray,
                   unsigned int *modearray, int returnall);
int FindLegHeatSinks(Mech *mech);
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
int FindWeaponIndex(Mech *mech, int number);
int findAmmoInSection(Mech *mech, int section, int type, int nogof, int gof);
int FullAmmo(const Mech *mech, int loc, int pos);
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
int CountAmmoForWeapon(Mech *mech, int weapindx);
int FindArtemisForWeapon(Mech *mech, int section, int critical);
int ReverseSplitCritLoc(Mech *mech, int sect, int crit);
int FindSplitCrits(Mech *mech, int sect, int type, int crit);
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
int FindRoundsForWeapon(Mech *mech, int weapindx);
int HeatFactor(Mech *mech);
int WeaponIsNonfunctional(Mech *mech, int section, int crit, int numcrits);
const char *const *ProperSectionStringFromType(int type, int mtype);
typedef struct UnitSectionCatalog {
  int unit_type;
  int movement_type;
} UnitSectionCatalog;

size_t unit_section_name_count(const UnitSectionCatalog *catalog);
const char *unit_section_name(const UnitSectionCatalog *catalog, size_t index);
void ArmorStringFromIndex(int index, char *buffer, UnitClass type,
                          MechMovementType movement_type);
int GetWeaponCrits(Mech *mech, int weapindx);
int listmatch(const char *const *values, size_t value_count, const char *match);
typedef struct MultiWeaponSelectionCall {
  Mech *mech;
  DbRef actor;
  int first;
  int last;
  void *context;
} MultiWeaponSelectionCall;
typedef int (*MultiWeaponSelectionCallback)(
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
void mech_RepairPart(Mech *mech, int loc, int pos);
int no_locations_destroyed(Mech *mech);
void mech_ReAttach(Mech *mech, int loc);
void mech_ReplaceSuit(Mech *mech, int loc);
void mech_ReSeal(Mech *mech, int loc);
void mech_Detach(Mech *mech, int loc);
void mech_FillPartAmmo(Mech *mech, int loc, int pos);

int CountDestroyedLegs(Mech *objMech);
int IsLegDestroyed(Mech *objMech, int wLoc);
int IsMechLegLess(Mech *objMech);
typedef struct WeaponCriticalSearch {
  Mech *mech;
  CriticalSlotReference weapon;
  int start_critical;
  int part_type;
  int maximum_criticals;
} WeaponCriticalSearch;
int mech_weapon_first_critical(const WeaponCriticalSearch *search);
int checkAllSections(Mech *mech, int specialToFind);
int checkSectionForSpecial(Mech *mech, int specialToFind, int wSec);
int getRemainingInternalPercent(Mech *mech);
int getRemainingArmorPercent(Mech *mech);
int FindObj(Mech *mech, int loc, int type);
int FindObjWithDest(Mech *mech, int loc, int type);
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

#ifdef BT_CALCULATE_BV
void Calc_AddOffBV(const Mech *mech, float *offbv, const char *desc,
                   float value);
void Calc_AddDefBV(const Mech *mech, float *defbv, const char *desc,
                   float value);
void Calc_SubDefBV(const Mech *mech, float *defbv, const char *desc,
                   float value);
#endif
int mech_armorpoints(Mech *mech);
int mech_intpoints(Mech *mech);
#ifdef BT_CALCULATE_BV
int FindAverageGunnery(Mech *mech);
int CalculateBV(Mech *mech, int gunstat, int pilstat);
float Calculate_Defensive_BV(Mech *mech);
float Calculate_Offensive_BV(Mech *mech);
#endif
void unit_parts_list(Mech *mech, char buffer[static LBUF_SIZE]);
int mech_recycling_state(Mech *mech, int num);
#ifdef BT_COMPLEXREPAIRS
int GetPartMod(const Mech *mech, int t);
int ProperArmor(const Mech *mech);
int ProperInternal(const Mech *mech);
int alias_part(Mech *mech, int t, int loc);
int ProperMyomer(Mech *mech);
#endif
