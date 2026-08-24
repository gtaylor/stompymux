local first_startup_count = 0
local startup_count = 0
local startup_order = {}

return {
  events = {
    on_server_first_startup = function(ctx)
      local object = mux.world.object(ctx.object)
      assert(ctx.enactor == 1 and ctx.cause == 1)
      assert(object.dbref == ctx.object and object.type == "player")
      assert(mux.world.lua_parent(object) == "default_player.lua")
      first_startup_count = first_startup_count + 1
      table.insert(startup_order, "first")
    end,
    on_server_startup = function(ctx)
      assert(mux.world.object(ctx.object).type == "player")
      startup_count = startup_count + 1
      table.insert(startup_order, "startup")
    end,
  },
  commands = {
    {
      pattern = "^objectevents$",
      handler = function(ctx)
        mux.world.pemit(
          ctx.enactor,
          "ObjectEvents first="
            .. first_startup_count
            .. " startup="
            .. startup_count
            .. " order="
            .. table.concat(startup_order, ",")
        )
        return true
      end,
    },
  },
}
