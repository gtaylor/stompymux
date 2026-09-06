local lines = {
  mux.text.markup("[color=cyan bold]OSC 8 demonstration (Tier 1-6)[/]"),
  mux.text.markup(
    'Tier 1 - basic actions: [link="https://wiki.mudlet.org/w/Manual:Supported_Protocols#OSC_8:_Hyperlink_Protocol"]web link[/] | [send="look"]send look[/] | [prompt="say OSC 8 works!"]fill input[/]'
  ),
  mux.text.markup(
    'Tier 2 - visual and state styling: [send="look" color=deepskyblue bg=midnightblue bold underline=wavy hover.color=yellow active.bg=darkred visited.color=mediumpurple focus-visible.underline=dashed]Styled look[/]'
  ),
  mux.text.markup(
    'Tier 3 - tooltip and context menu: [send="look" color=white bg=teal bold hover.bg=darkcyan tooltip="Left-click to look; right-click for more actions" title="OSC 8 actions" title.color=aqua title.bold menu.1.label="Look" menu.1.send="look" menu.2.label="Prepare examine" menu.2.prompt="examine me" menu.3.separator menu.4.label="Mudlet OSC 8 manual" menu.4.link="https://wiki.mudlet.org/w/Manual:Supported_Protocols#OSC_8:_Hyperlink_Protocol"]Interactive actions[/]'
  ),
  mux.text.markup(
    'Tier 4 - visibility: [send="look" visibility.action=conceal visibility.delay=500 tooltip="Click to hide after half a second"]Click to dismiss[/] | [send="look" visibility.action=reveal visibility.delay=3000 tooltip="This appears after three seconds"]Delayed reveal[/]'
  ),
  mux.text.markup('Tier 4 - spoiler: [send="look" spoiler]The answer is 42[/]'),
  mux.text.markup('Tier 4 - reveal-only spoiler: [send="look" spoiler disabled]A secret with no follow-up action[/]'),
  mux.text.markup(
    'Tier 4 - disabled link: [send="look" disabled color=gray disabled.color=darkgray disabled.strikethrough tooltip="This action is unavailable"]Locked action[/]'
  ),
  mux.text.markup(
    'Tier 5 - radio choices: [send="say difficulty easy" selection.group="difficulty" selection.value="easy" selection.exclusive selection.selected selected.color=lime selected.bold]Easy[/] | [send="say difficulty hard" selection.group="difficulty" selection.value="hard" selection.exclusive selected.color=red selected.bold]Hard[/]'
  ),
  mux.text.markup(
    'Tier 5 - checkbox choices: [send="say strength toggled" selection.group="buffs" selection.value="strength" selection.exclusive=false selected.bg=darkgreen selected.bold]Strength[/] | [send="say speed toggled" selection.group="buffs" selection.value="speed" selection.exclusive=false selection.selected selected.bg=darkblue selected.bold]Speed[/]'
  ),
  mux.text.markup(
    'Tier 5 - fixed states: [send="say following remains enabled" selection.group="following" selection.value="news" selection.toggle=false selection.selected selected.color=cyan]Following[/] | [send="say unavailable" selection.group="poll" selection.value="closed" selection.disabled disabled.color=gray disabled.strikethrough]Closed option[/]'
  ),
  mux.text.markup(
    'Tier 6 - presets: [send="say preset button" preset="osc8-demo-button" tooltip="A shared button preset"]Preset button[/] | [send="say preset override" preset="osc8-demo-button" bg=blue tooltip="This link overrides the shared background"]Blue override[/] | [send="say danger" preset="osc8-demo-danger"]Danger[/]'
  ),
  "Tier 6-capable clients receive compact JSON and each configured preset once per connection.",
  "Hover and focus the styled links, activate them, and right-click the Tier 3 link.",
}

return {
  commands = {
    {
      pattern = "^[Oo][Ss][Cc]8[Dd][Ee][Mm][Oo]$",
      handler = function(ctx)
        for _, line in ipairs(lines) do
          mux.world.pemit(ctx.enactor, line)
        end
        return true
      end,
    },
  },
}
