-- Globally available after @luareload.
-- Demonstrates the three shapes an interactive flow can take: a single-step
-- yes/no confirmation, a numbered menu with a goto, and a multi-step form
-- that carries data across steps via ctx.flow. Type "flow-demo confirm",
-- "flow-demo menu", or "flow-demo signup" to try one.
return {
  commands = {
    {
      pattern = "^flow%-demo%s+(%S+)$",
      handler = function(ctx, choice)
        local starters = { confirm = "confirm", menu = "menu", signup = "signup_name" }
        local first_step = starters[choice]
        if not first_step then
          mux.notify(ctx.enactor, "Usage: flow-demo confirm|menu|signup")
          return true
        end
        mux.flow_start(ctx.descriptor, "flow_examples.lua", first_step)
        return true
      end,
    },
  },

  flows = {
    confirm = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt = "Really do the thing? (y/n) " }
      end
      if ctx.input:match("^[Yy]") then
        return { action = "done", message = "Done." }
      elseif ctx.input:match("^[Nn]") then
        return { action = "cancel", message = "Cancelled." }
      end
      return { action = "wait", prompt = "Please answer y or n: " }
    end,

    menu = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt =
          "1) Say hello\r\n2) Say goodbye\r\n3) Cancel\r\nChoice: " }
      end
      if ctx.input == "1" then
        return { action = "goto", step = "menu_hello" }
      elseif ctx.input == "2" then
        return { action = "goto", step = "menu_goodbye" }
      elseif ctx.input == "3" then
        return { action = "cancel", message = "Nevermind, then." }
      end
      return { action = "wait", prompt = "Enter 1, 2, or 3: " }
    end,
    menu_hello = function(ctx)
      return { action = "done", message = "Hello there!" }
    end,
    menu_goodbye = function(ctx)
      return { action = "done", message = "Farewell!" }
    end,

    signup_name = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt = "What's your character concept's name? " }
      end
      ctx.flow.name = ctx.input
      return { action = "goto", step = "signup_faction" }
    end,
    signup_faction = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt =
          "Faction -- (1) Inner Sphere or (2) Clan? " }
      end
      if ctx.input ~= "1" and ctx.input ~= "2" then
        return { action = "wait", prompt = "Enter 1 or 2: " }
      end
      ctx.flow.faction = (ctx.input == "1") and "Inner Sphere" or "Clan"
      return { action = "goto", step = "signup_confirm" }
    end,
    signup_confirm = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt = string.format(
          "%s, %s -- confirm? (y/n) ", ctx.flow.name, ctx.flow.faction) }
      end
      if ctx.input:match("^[Yy]") then
        return { action = "done", message = string.format(
          "Recorded %s (%s).", ctx.flow.name, ctx.flow.faction) }
      end
      return { action = "goto", step = "signup_name" }
    end,
  },
}
