typedef enum : int { STATUS_NONE = 0, STATUS_ACTIVE = 1 } MechStatus;

bool mech_status_has(MechStatus status, MechStatus flag) {
  return ((int)status & (int)flag) != 0;
}

bool status_is_active(MechStatus status) {
  return mech_status_has(status, STATUS_ACTIVE);
}
