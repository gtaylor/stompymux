#include "values_internal.h"

int main(void) {
  int first = 0;
  int second = 0;
  int third = 0;

  if (!parse_damage_numbers("A:3/7", "A:", false, &first, &second, &third) ||
      first != 3 || second != 7)
    return 1;

  if (!parse_damage_numbers("G:2/4(5)", "G:", true, &first, &second, &third) ||
      first != 2 || second != 4 || third != 5)
    return 2;
  return 0;
}
