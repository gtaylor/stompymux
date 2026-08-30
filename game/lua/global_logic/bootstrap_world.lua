-- Limbo: #0
-- God: #1
-- Wizard: #2
-- Used mech store: #3
-- Player starting room: #4
-- Player starting home: #4
-- Afterlife room: #5

local function bootstrap(ctx)
  local limbo = mux.world.object(0)
  limbo:set_name("Staff Nexus")

  local room = mux.world.create_room({ name = "Test Room" })
  room:set_zone(0)
  room:attributes():set("Description", "This is a test room created during world bootstrap.\n\nNewline test")
end

return {
  events = {
    on_server_first_startup = bootstrap,
  },
  commands = {
    {
      pattern = "^re%-bootstrap$",
      access = mux.world.access.WIZARD,
      handler = function(ctx)
        mux.world.pemit(ctx.enactor, "Hello, world, from a global Lua command!")
        return true
      end,
    },
  },
}
