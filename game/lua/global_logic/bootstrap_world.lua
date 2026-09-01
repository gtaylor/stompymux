-- Limbo: #0
-- God: #1
-- Wizard: #2
-- Used mech store: #3
-- Player starting room: #4
-- Player starting home: #4
-- Afterlife room: #5

local function bootstrap_channels(ctx, staff_rooms)
  local public_channel_object = mux.world.create_object({
    type = mux.world.types.THING,
    name = "Channel object: Public",
    location = staff_rooms.channel_storage_room,
    zone = staff_rooms.limbo_room,
  })
  local public_channel = mux.comsys.create_channel("Public")
  public_channel:set_object(public_channel_object)
  public_channel:flags():add(mux.comsys.flags.PUBLIC)

  local staff_channel_object = mux.world.create_object({
    type = mux.world.types.THING,
    name = "Channel object: Staff",
    location = staff_rooms.channel_storage_room,
    zone = staff_rooms.limbo_room,
  })
  local staff_channel = mux.comsys.create_channel("Staff")
  staff_channel:set_object(staff_channel_object)
end

local function bootstrap_players(ctx)
  -- TODO: Add God and Wizard to channels
  local god_player = mux.world.object(1)
  -- TODO: Set God and Wizard's home to #0
  local wizard_player = mux.world.object(2)
end

local function bootstrap_staff_rooms(ctx)
  local limbo_room = mux.world.object(0)
  limbo_room:set_name("Staff Nexus")
  limbo_room:attributes():set(
    "Description",
    [[This is the hub of the guts of the MUX. From here you can get to the internals of the game, sorted into various different rooms.

Please be sure to read the room descriptions for details on the contents of each area.
]])

  local new_player_starting_room = mux.world.object(mux.config.get("player_starting_room") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "New Player Room;np",
    location = limbo_room,
    zone = limbo_room,
    destination = new_player_starting_room,
  })

  local used_mech_store_room = mux.world.object(mux.config.get("btech_usedmechstore") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Used Mech Store;us;ums",
    location = limbo_room,
    zone = limbo_room,
    destination = used_mech_store_room,
  })
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Out;o",
    location = used_mech_store_room,
    zone = limbo_room,
    destination = limbo_room,
  })

  local afterlife_room = mux.world.object(mux.config.get("btech_afterlife_dbref") --[[@as integer]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Afterlife;al",
    location = limbo_room,
    zone = limbo_room,
    destination = afterlife_room,
  })

  local channel_storage_room = mux.world.create_object({
    type = mux.world.types.ROOM,
    name = "Channel Object Storage",
    zone = limbo_room,
  })
  channel_storage_room:attributes():set(
    "Description",
    [[This room contains channel objects, which are attached to comsys channels and are used for advanced configuration and locking.]])
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Channel Object Storage;cs;cos",
    location = limbo_room,
    zone = limbo_room,
    destination = channel_storage_room,
  })
  mux.world.create_object({
    type = mux.world.types.EXIT,
    name = "Out;o",
    location = channel_storage_room,
    zone = limbo_room,
    destination = limbo_room,
  })
  return {
    limbo_room = limbo_room,
    channel_storage_room = channel_storage_room,
  }
end

local function bootstrap_player_rooms(ctx)
  local new_player_starting_room = mux.world.object(mux.config.get("player_starting_room") --[[@as integer]])
  new_player_starting_room:attributes():set(
    "Description",
    [[Welcome to this new StompyMUX game. We're not ready for prime time quite yet but you are Welcome to explore if you'd like.]]
  )
end

local function bootstrap(ctx)
  local staff_rooms = bootstrap_staff_rooms(ctx)
  bootstrap_player_rooms(ctx)
  bootstrap_channels(ctx, staff_rooms)
  bootstrap_players(ctx)
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
          mux.world.destroy_object(object, { override = true })
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
