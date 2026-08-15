#include <assert.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_path_policy_api.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "section_types.h"

static int autopilot_hex_offset(int x, int y) { return (x * MAPY) + y; }

static short autopilot_map_coordinate(int coordinate) {
  assert(coordinate >= SHRT_MIN && coordinate <= SHRT_MAX);
  return (short)coordinate;
}

static void *astar_key(int value) { return (void *)(intptr_t)value; }

typedef struct AutopilotHexBitSet {
  unsigned char bytes[(MAPX * MAPY) / 8];
} AutopilotHexBitSet;

static unsigned char *autopilot_hex_bit_byte(AutopilotHexBitSet *bits,
                                             int offset) {
  if (offset < 0 || offset >= MAPX * MAPY)
    abort();
  return checked_storage_at(bits->bytes, sizeof(bits->bytes),
                            sizeof(unsigned char), (size_t)offset >> 3);
}

static bool autopilot_hex_bit_is_set(AutopilotHexBitSet *bits, int offset) {
  const unsigned char BYTE = *autopilot_hex_bit_byte(bits, offset);
  return (BYTE & (1U << (offset & 7))) != 0;
}

static void autopilot_hex_bit_set(AutopilotHexBitSet *bits, int offset,
                                  bool enabled) {
  unsigned char *byte = autopilot_hex_bit_byte(bits, offset);
  const unsigned char MASK = (unsigned char)(1U << (offset & 7));
  if (enabled)
    *byte |= MASK;
  else
    *byte &= (unsigned char)~MASK;
}

/* Experimental (highly) path finding system based on the A* 'a-star'
 * system used in many typical games.
 *
 * Dany - 08/2005 */

/*
 * Create an astar node and return a pointer to it
 */
typedef struct AutopilotPathCoordinate {
  short x;
  short y;
} AutopilotPathCoordinate;

typedef struct AutopilotPathNodeRequest {
  AutopilotPathCoordinate position;
  AutopilotPathCoordinate parent;
  int path_score;
  int heuristic_score;
} AutopilotPathNodeRequest;

static AutopilotPathNode *
auto_create_astar_node(const AutopilotPathNodeRequest *request) {

  AutopilotPathNode *temp;
  temp = checked_storage_try_allocate(sizeof(AutopilotPathNode));
  if (temp == nullptr)
    return nullptr;

  memset(temp, 0, sizeof(AutopilotPathNode));

  temp->x = request->position.x;
  temp->y = request->position.y;
  temp->x_parent = request->parent.x;
  temp->y_parent = request->parent.y;
  temp->g_score = request->path_score;
  temp->h_score = request->heuristic_score;
  temp->f_score = request->path_score + request->heuristic_score;
  temp->hexoffset = (request->position.x * MAPY) + request->position.y;

  return temp;
}

/*
 * The A* (A-Star) path finding function for the AI
 *
 * Returns 1 if it found a path and 0 if it doesn't
 */
static int astar_compare(const RedBlackTreeCompareCall *call) {
  const void *left_key = call->lhs;
  const void *right_key = call->rhs;
  const intptr_t LEFT = (intptr_t)left_key;
  const intptr_t RIGHT = (intptr_t)right_key;

  return (LEFT > RIGHT) - (LEFT < RIGHT);
}
static void astar_release(const RedBlackTreeReleaseCall *call) {
  [[maybe_unused]] void *key = call->key;
  void *data = call->data;
  [[maybe_unused]] void *arg = call->context;
  (void)key;
  (void)arg;
  free(data);
}

static AutopilotPathMobility autopilot_path_mobility(const Mech *mech) {
  if (mech_class(mech) == CLASS_MECH)
    return AUTOPILOT_PATH_MECH;
  if (mech_class(mech) != CLASS_VEH_GROUND)
    return AUTOPILOT_PATH_OTHER;
  if (mech_movement_type(mech) == MOVE_TRACK)
    return AUTOPILOT_PATH_TRACKED;
  if (mech_movement_type(mech) == MOVE_HOVER)
    return AUTOPILOT_PATH_HOVER;
  return AUTOPILOT_PATH_WHEELED;
}

int auto_astar_generate_path(Autopilot *autopilot, Mech *mech, int end_x,
                             int end_y) {
  BattleMap *map =
      btech_context_get_map(autopilot->xcode.context, autopilot->mapindex);
  int found_path = 0;

  /* Our bit arrays */
  AutopilotHexBitSet closed_list_bitfield = {0};
  AutopilotHexBitSet open_list_bitfield = {0};

  float x1;
  float y1;
  float x2;
  float y2; /* Floating point vars for real cords */
  short map_x1;
  short map_y1;
  short map_x2;
  short map_y2; /* The actual map 'hexes' */
  int i;
  int child_g_score;
  int child_h_score; /* the score values for the child hexes */
  int hexoffset;     /* temp int to pass around as the hexoffset */

  /* Our lists using Hag's RedBlackTree */
  /* Using two RedBlackTree's to store the open_list so we can sort two
   * different ways */
  RedBlackTree open_list_by_score; /* open list sorted by score */
  RedBlackTree open_list_by_xy;    /* open list sorted by hexoffset */
  RedBlackTree closed_list;        /* closed list sorted by hexoffset */

  /* Helper node for the final path */
  DoublyLinkedListNode *astar_path_node;

  /* Our AutopilotPathNode helpers */
  AutopilotPathNode *temp_astar_node;
  AutopilotPathNode *parent_astar_node;

  /* Setup the trees */
  open_list_by_score = red_black_tree_init(astar_compare, nullptr);
  open_list_by_xy = red_black_tree_init(astar_compare, nullptr);
  closed_list = red_black_tree_init(astar_compare, nullptr);

  /* Setup the path */
  /* Destroy any existing path first */
  auto_destroy_astar_path(autopilot);
  autopilot->astar_path = doubly_linked_list_create_list();

  /* Setup the start hex */
  temp_astar_node = auto_create_astar_node(&(AutopilotPathNodeRequest){
      .position = {.x = autopilot_map_coordinate(mech_position_x(mech)),
                   .y = autopilot_map_coordinate(mech_position_y(mech))},
      .parent = {.x = -1, .y = -1}});

  if (temp_astar_node == nullptr) {
    /*! \todo {Add code here to break if we can't alloc memory} */
  }

  /* Add start hex to open list */
  red_black_tree_insert(open_list_by_score, astar_key(temp_astar_node->f_score),
                        temp_astar_node);
  red_black_tree_insert(open_list_by_xy, astar_key(temp_astar_node->hexoffset),
                        temp_astar_node);
  autopilot_hex_bit_set(&open_list_bitfield, temp_astar_node->hexoffset, true);

  /* Now loop till we find path */
  while (!found_path) {

    /* Check to make sure there is still stuff in the open list
     * if not, means we couldn't find a path so quit */
    if (red_black_tree_size(open_list_by_score) == 0) {
      break;
    }

    /* Get lowest cost node, then remove it from the open list */
    parent_astar_node = (AutopilotPathNode *)red_black_tree_search(
        open_list_by_score, SEARCH_FIRST, nullptr);

    red_black_tree_delete(open_list_by_score,
                          astar_key(parent_astar_node->f_score));
    red_black_tree_delete(open_list_by_xy,
                          astar_key(parent_astar_node->hexoffset));
    autopilot_hex_bit_set(&open_list_bitfield, parent_astar_node->hexoffset,
                          false);

    /* Add it to the closed list */
    red_black_tree_insert(closed_list, astar_key(parent_astar_node->hexoffset),
                          parent_astar_node);
    autopilot_hex_bit_set(&closed_list_bitfield, parent_astar_node->hexoffset,
                          true);

    /* Now we check to see if we added the end hex to the closed list.
     * When this happens it means we are done */
    if (autopilot_hex_bit_is_set(&closed_list_bitfield,
                                 autopilot_hex_offset(end_x, end_y))) {
      found_path = 1;

      break;
    }

    /* Update open list */
    /* Loop through the hexes around current hex and see if we can add
     * them to the open list */

    /* Set the parent hex of the new nodes */
    map_x1 = parent_astar_node->x;
    map_y1 = parent_astar_node->y;

    /* Going around clockwise direction */
    for (i = 0; i < 360; i += 60) {

      /* Map coord to Real */
      map_coord_to_real_coord(map_x1, map_y1, &x1, &y1);

      /* Calc new hex */
      MapRealPosition projected = map_project_position(&(MapProjection){
          .origin = {.x = x1, .y = y1}, .bearing = i, .range = 1.0F});
      x2 = projected.x;
      y2 = projected.y;

      /* Real coord to Map */
      real_coord_to_map_coord(&map_x2, &map_y2, x2, y2);

      /* Make sure the hex is sane */
      if (map_x2 < 0 || map_y2 < 0 || map_x2 >= battle_map_width(map) ||
          map_y2 >= battle_map_height(map))
        continue;

      /* Generate hexoffset for the child node */
      hexoffset = autopilot_hex_offset(map_x2, map_y2);

      /* Check to see if its in the closed list
       * if so just ignore it */
      if (autopilot_hex_bit_is_set(&closed_list_bitfield, hexoffset))
        continue;

      const int FRIENDLY_UNITS =
          battle_map_mech_count_in_hex(&(BattleMapHexOccupancyRequest){
              .map = map,
              .position = {.x = map_x2, .y = map_y2},
              .relationship = TEAM_RELATIONSHIP_FRIENDLY,
              .team = mech_team(mech)});
      const AutopilotPathStepResult STEP =
          autopilot_path_step_evaluate(&(AutopilotPathStepRequest){
              .mobility = autopilot_path_mobility(mech),
              .waterproof = (mech_technology_flags_secondary(mech) &
                             WATERPROOF_TECH) != 0,
              .from = {.terrain = map_terrain_get(map, map_x1, map_y1),
                       .elevation = map_elevation_get(map, map_x1, map_y1)},
              .to = {.terrain = map_terrain_get(map, map_x2, map_y2),
                     .elevation = map_elevation_get(map, map_x2, map_y2),
                     .friendly_units = FRIENDLY_UNITS}});
      if (!STEP.traversable)
        continue;
      child_g_score = STEP.cost;

      /* Now add the g score from the parent */
      child_g_score += parent_astar_node->g_score;

      /* Next get range */
      /* Using a varient of the Manhattan method since its perfectly
       * logical for us to go diagonally
       *
       * Basicly just going to get the range,
       * and multiply by 100 */
      /*! \todo {Add in something for elevation cost} */

      /* Get the end hex in real coords, using the old variables
       * to store the values */
      map_coord_to_real_coord(end_x, end_y, &x1, &y1);

      /* Re-using the x2 and y2 values we calc'd for the child hex
       * to find the range between the child hex and end hex */
      const float ESTIMATED_COST = 100.0F * map_real_range(&(MapRealSegment){
                                                .start = {.x = x2, .y = y2},
                                                .end = {.x = x1, .y = y1},
                                            });
      child_h_score = (int)ESTIMATED_COST;

      /* Is it already on the openlist */
      if (autopilot_hex_bit_is_set(&open_list_bitfield, hexoffset)) {

        /* Ok need to compare the scores and if necessary recalc
         * and change stuff */

        /* Get the node off the open_list */
        temp_astar_node = (AutopilotPathNode *)red_black_tree_find(
            open_list_by_xy, astar_key(hexoffset));

        /* Now compare the 'g_scores' to determine shortest path */
        /* If g_score is lower, this means better path
         * from the current parent node */
        if (child_g_score < temp_astar_node->g_score) {

          /* Remove from open list */
          red_black_tree_delete(open_list_by_score,
                                astar_key(temp_astar_node->f_score));
          red_black_tree_delete(open_list_by_xy,
                                astar_key(temp_astar_node->hexoffset));
          autopilot_hex_bit_set(&open_list_bitfield, temp_astar_node->hexoffset,
                                false);

          /* Recalc score */
          /* H-Score should be the same since the hex doesn't move */
          temp_astar_node->g_score = child_g_score;
          temp_astar_node->f_score =
              temp_astar_node->g_score + temp_astar_node->h_score;

          /* Change parent hex */
          temp_astar_node->x_parent = map_x1;
          temp_astar_node->y_parent = map_y1;

          /* Will re-add the node below */

        } else {

          /* Don't need to do anything so we can skip
           * to the next node */
          continue;
        }

      } else {

        /* Node isn't on the open list so we have to create it */
        temp_astar_node = auto_create_astar_node(
            &(AutopilotPathNodeRequest){.position = {.x = map_x2, .y = map_y2},
                                        .parent = {.x = map_x1, .y = map_y1},
                                        .path_score = child_g_score,
                                        .heuristic_score = child_h_score});

        if (temp_astar_node == nullptr) {
          /*! \todo {Add code here to break if we can't alloc memory} */
        }
      }

      /* Now add (or re-add) the node to the open list */

      /* Hack to check to make sure its score is not already on the open
       * list. This slightly skews the results towards nodes found earlier
       * then those found later */
      while (1) {

        if (red_black_tree_exists(open_list_by_score,
                                  astar_key(temp_astar_node->f_score))) {
          temp_astar_node->f_score++;

        } else {
          break;
        }
      }
      red_black_tree_insert(open_list_by_score,
                            astar_key(temp_astar_node->f_score),
                            temp_astar_node);
      red_black_tree_insert(open_list_by_xy,
                            astar_key(temp_astar_node->hexoffset),
                            temp_astar_node);
      autopilot_hex_bit_set(&open_list_bitfield, temp_astar_node->hexoffset,
                            true);

    } /* End of looking for hexes next to us */

  } /* End of looking for path */

  /* We Done lets go */

  /* Lets first see if we found a path */
  if (found_path) {

    /* Found a path so we need to go through the closed list
     * and generate it */

    /* Get the end hex, find its parent hex and work back to
     * start hex while building list */

    /* Get end hex from closed list */
    hexoffset = autopilot_hex_offset(end_x, end_y);
    temp_astar_node = red_black_tree_find(closed_list, astar_key(hexoffset));

    /* Add end hex to path list */
    astar_path_node = doubly_linked_list_create_node(temp_astar_node);
    doubly_linked_list_insert_beginning(autopilot->astar_path, astar_path_node);

    /* Remove it from closed list */
    red_black_tree_delete(closed_list, astar_key(temp_astar_node->hexoffset));

    /* Check if the end hex is the start hex */
    if (!(temp_astar_node->x == mech_position_x(mech) &&
          temp_astar_node->y == mech_position_y(mech))) {

      /* Its not so lets loop through the closed list
       * building the path */

      /* Loop */
      while (1) {

        /* Get Parent Node Offset */
        hexoffset = autopilot_hex_offset(temp_astar_node->x_parent,
                                         temp_astar_node->y_parent);

        /*! \todo {Possibly add check here incase the node we're
         * looking for some how did not end up on the list} */

        /* Get Parent Node from closed list */
        parent_astar_node =
            red_black_tree_find(closed_list, astar_key(hexoffset));

        /* Check if start hex */
        /* If start hex quit */
        if (parent_astar_node->x == mech_position_x(mech) &&
            parent_astar_node->y == mech_position_y(mech)) {
          break;
        }

        /* Add to path list */
        astar_path_node = doubly_linked_list_create_node(parent_astar_node);
        doubly_linked_list_insert_beginning(autopilot->astar_path,
                                            astar_path_node);

        /* Remove from closed list */
        red_black_tree_delete(closed_list,
                              astar_key(parent_astar_node->hexoffset));

        /* Make parent new child */
        temp_astar_node = parent_astar_node;

      } /* End of while loop */
    }

    /* Done with the path its cleanup time */
  }

  /* Make sure we destroy all the objects we dont need any more */

  /* Destroy the open lists */
  red_black_tree_release(open_list_by_score, astar_release, nullptr);
  red_black_tree_destroy(open_list_by_xy);

  /* Destroy the closed list */
  red_black_tree_release(closed_list, astar_release, nullptr);

  /* End */
  if (found_path) {
    return 1;
  }
  return 0;
}

void auto_destroy_astar_path(Autopilot *autopilot) {

  AutopilotPathNode *temp_astar_node;

  /* Make sure there is a path if not quit */
  if (!(autopilot->astar_path))
    return;

  /* There is a path lets kill it */
  if (doubly_linked_list_size(autopilot->astar_path) > 0) {

    while (doubly_linked_list_size(autopilot->astar_path)) {
      temp_astar_node =
          doubly_linked_list_remove_node_at_pos(autopilot->astar_path, 1);
      free(temp_astar_node);
    }
  }

  /* Finally destroying the path */
  doubly_linked_list_destroy_list(autopilot->astar_path);
  autopilot->astar_path = nullptr;
}
