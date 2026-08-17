int item_count(int item) { return item + 2; }

int safe_copy_chr(int item) { return item != 0; }

typedef void RepairOperationCall(void);
typedef void ConfigurationCall(void);
typedef struct lua_State lua_State;

int repair_callback_is_ready(RepairOperationCall *callback) {
  return callback != nullptr;
}

int configuration_callback_is_ready(ConfigurationCall *callback) {
  return callback != nullptr;
}

int lua_state_is_ready(lua_State *state) { return state != nullptr; }
