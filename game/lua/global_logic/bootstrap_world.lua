-- Limbo: #0
-- God: #1
-- Wizard: #2
-- Used mech store: #3
-- Player starting room: #4
-- Player starting home: #4
-- Afterlife room: #5

local function bootstrap_channels(ctx)
  -- TODO: Implement!
end

local function bootstrap_players(ctx)
  -- TODO: Add God and Wizard to channels
  -- TODO: Set God and Wizard's home to #0
end

local function bootstrap_rooms(ctx)
  local limbo = mux.world.object(0)
  limbo:set_name("Staff Nexus")
  limbo:attributes():set(
    "Description",
    [[This is the hub of the guts of the MUX. From here you can get to the internals of the game, sorted into various different rooms.

Please be sure to read the room descriptions for details on the contents of each area.
]]
  )
  local new_player_starting_room = mux.world.object(mux.config.get("player_starting_room") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "New Player Room;np",
    location = limbo,
    zone = limbo,
    destination = new_player_starting_room,
  })

  local used_mech_store_room = mux.world.object(mux.config.get("btech_usedmechstore") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Used Mech Store;us;ums",
    location = limbo,
    zone = limbo,
    destination = used_mech_store_room,
  })

  local afterlife_room = mux.world.object(mux.config.get("btech_afterlife_dbref") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Afterlife;al",
    location = limbo,
    zone = limbo,
    destination = afterlife_room,
  })

  new_player_starting_room:attributes():set(
    "Description",
    [[Welcome to this new StompyMUX game. We're not ready for prime time quite yet but you are Welcome to explore if you'd like.
]]
  )

  local room = mux.world.create_object({ type = mux.world.types.ROOM, name = "Test Room", zone = limbo })
  room:attributes():set(
    "Description",
    [[This is a test room created during world bootstrap.

Newline test
]]
  )
end

local function bootstrap(ctx)
  bootstrap_channels(ctx)
  bootstrap_players(ctx)
  bootstrap_rooms(ctx)
end

return {
  events = {
    on_server_first_startup = bootstrap,
  },
  commands = {
    {
      pattern = "^bs%-delete$",
      access = mux.world.access.WIZARD,
      handler = function(ctx)
        mux.world.pemit(ctx.enactor, "Deleting all bootstrapped objects.")
        local bootstrapped_objects = mux.world.list_objects({
          in_zone = 0,
        })
        for _, object in ipairs(bootstrapped_objects) do
          mux.world.destroy(object, { override = true })
        end

        mux.check_db()
        mux.world.pemit(ctx.enactor, "Deletion complete.")
        return true
      end,
    },
    {
      pattern = "^bs%-run$",
      access = mux.world.access.WIZARD,
      handler = function(ctx)
        mux.world.pemit(ctx.enactor, "Re-bootstrapping!")
        bootstrap(ctx)
        mux.world.pemit(ctx.enactor, "Re-bootstrapping completed.")
        return true
      end,
    },
  },
}
