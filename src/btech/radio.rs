//! Saved channel settings with capabilities derived from the installed radio quality.
use super::Mech;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Hardware limits shared by all supported radios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RadioCapabilities {
    pub channels: u8,
    pub range: u16,
    pub relay: bool,
    pub info: bool,
    pub scan: bool,
    pub digital: bool,
}

impl RadioCapabilities {
    /// Encode the installed channel count and hardware feature bits for administrative inspection.
    pub(super) fn configuration(self) -> u8 {
        self.channels
            | (u8::from(self.relay) * 16)
            | (u8::from(self.info) * 32)
            | (u8::from(self.scan) * 64)
            | (u8::from(!self.digital) * 128)
    }
}

/// Transmission and reception preferences, independent of hardware and channel frequency.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadioMode {
    pub digital: bool,
    pub muted: bool,
    pub relay: bool,
    #[serde(default)]
    pub info: bool,
    #[serde(default)]
    pub scan: bool,
    pub color: Option<char>,
}

/// One radio channel; inactive slots remain saved if equipment configuration changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadioChannel {
    pub frequency: u32,
    pub title: String,
    pub mode: RadioMode,
}

/// Communication target for a constructed radio before startup samples its pilot.
pub(super) fn default_skill() -> i16 {
    6
}

impl Mech {
    /// Saved communication target used by reception interference until the next startup.
    pub fn radio_skill(&self) -> i16 {
        self.radio_skill
    }

    /// Quality zero uses the chassis default; nonzero radio range overrides its derived reach.
    pub fn radio_capabilities(&self) -> RadioCapabilities {
        capabilities(
            &self.definition().attributes,
            self.definition().has_special("Clan"),
            self.hardware,
        )
    }

    /// Active channel settings in letter order, starting with channel A.
    pub fn radio_channels(&self) -> &[RadioChannel] {
        &self.radio[..usize::from(self.radio_capabilities().channels)]
    }

    /// Validate persisted settings, including slots currently hidden by the installed hardware.
    pub(super) fn validate_radio(&self) -> Result<()> {
        validate_channels(&self.radio)?;
        validate_attributes(&self.definition().attributes)
    }
}

/// Derive hardware capabilities from template attributes for every chassis.
fn capabilities(
    attributes: &std::collections::BTreeMap<String, String>,
    clan: bool,
    hardware: super::hardware_settings::HardwareSettings,
) -> RadioCapabilities {
    let quality = attributes
        .get("radio")
        .and_then(|value| value.parse::<u8>().ok())
        .filter(|quality| *quality != 0)
        .unwrap_or(if clan { 5 } else { 3 });
    let (channels, range) = match quality {
        1 => (2, 64),
        2 => (4, 80),
        3 => (5, 100),
        4 => (8, 120),
        5 => (11, 140),
        _ => (0, 0),
    };
    let configuration = attributes
        .get("radiotype")
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|v| *v != 0)
        .unwrap_or(channels + if clan || quality >= 4 { 16 } else { 0 });
    let configuration = hardware.radio_configuration.unwrap_or(configuration);
    RadioCapabilities {
        channels: configuration % 16,
        range: hardware.radio_range.unwrap_or_else(|| {
            attributes
                .get("radio_range")
                .and_then(|value| value.parse::<u16>().ok())
                .filter(|range| *range != 0)
                .unwrap_or(range)
        }),
        relay: configuration & 16 != 0,
        info: configuration & 32 != 0,
        scan: configuration & 64 != 0,
        digital: configuration & 128 == 0,
    }
}

/// Validate all saved slots, including those hidden by the current hardware.
pub(super) fn validate_channels(channels: &[RadioChannel; 16]) -> Result<()> {
    for channel in channels {
        ensure!(channel.frequency <= 999999, "Invalid radio frequency");
        ensure!(
            channel.title.len() <= 15,
            "Radio title exceeds fifteen bytes"
        );
        channel.mode.validate()?;
    }
    Ok(())
}

impl RadioMode {
    /// Decode the cockpit mode letters; an unrecognized suffix ends mode selection.
    pub fn parse(text: &str, capabilities: RadioCapabilities) -> Result<Self> {
        let mut mode = Self::default();
        for c in text.trim_start().chars() {
            match c {
                'D' | 'd' => {
                    ensure!(
                        capabilities.digital,
                        "Your radio can't handle digital frequencies!"
                    );
                    mode.digital = true;
                }
                'U' | 'u' => mode.muted = true,
                'E' | 'e' => {
                    ensure!(capabilities.relay, "This unit is unable to relay.");
                    mode.relay = true;
                }
                'I' | 'i' => {
                    ensure!(
                        capabilities.info,
                        "This unit is unable to use info functionality."
                    );
                    mode.info = true;
                }
                'S' | 's' => {
                    ensure!(capabilities.scan, "This unit is unable to scan.");
                    mode.scan = true;
                }
                color if "xrgybmcwXRGYBMCW".contains(color) => mode.color = Some(color),
                _ => break,
            }
        }
        mode.validate()?;
        Ok(mode)
    }

    /// Relay requires digital transmission; color identities are bounded independently of parsing.
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.relay || self.digital,
            "Error: Need digital transfer for relay to work."
        );
        ensure!(
            !self.info || self.digital,
            "Error: Need digital transfer for transfer info to work."
        );
        ensure!(
            self.color
                .is_none_or(|color| "xrgybmcwXRGYBMCW".contains(color)),
            "Invalid radio color"
        );
        Ok(())
    }
}

/// Check cockpit ownership and the active zero-based channel before changing settings.
fn access(world: &World, unit: ObjectId, pilot: ObjectId, channel: u8) -> Result<()> {
    // Channel controls remain available while shut down, under the common cockpit rules.
    super::power::controlled(world, unit, pilot)?;
    ensure!(
        channel < self::unit(world, unit)?.radio_capabilities().channels,
        "Invalid channel-letter!"
    );
    Ok(())
}

/// Native frequency syntax accepts decimal digits within the reference signed-integer parser.
/// The shared setter separately enforces the tunable range, preserving its distinct reply.
fn parse_frequency(value: &str) -> Result<u32> {
    ensure!(
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        "Invalid frequency!"
    );
    let value = value
        .parse::<i32>()
        .map_err(|_| anyhow::anyhow!("Invalid frequency!"))?;
    Ok(u32::try_from(value).expect("decimal digits cannot represent a negative frequency"))
}

/// Set a frequency without transmitting or triggering command mines.
pub fn set_radio_frequency(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    channel: u8,
    frequency: u32,
) -> Result<()> {
    access(world, unit, pilot, channel)?;
    ensure!(
        frequency <= 999999,
        "Invalid frequency - range is from 0 to 999999."
    );
    storage(world, unit)?.radio[usize::from(channel)].frequency = frequency;
    Ok(())
}

/// Save at most fifteen title bytes, stopping at a UTF-8 boundary rather than splitting a character.
pub fn set_radio_title(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    channel: u8,
    title: &str,
) -> Result<()> {
    access(world, unit, pilot, channel)?;
    let mut end = title.len().min(15);
    while !title.is_char_boundary(end) {
        end -= 1;
    }
    storage(world, unit)?.radio[usize::from(channel)].title = title[..end].into();
    Ok(())
}

/// Replace mode flags after validating both the channel and installed relay capability.
pub fn set_radio_mode(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    channel: u8,
    mode: RadioMode,
) -> Result<()> {
    access(world, unit, pilot, channel)?;
    mode.validate()?;
    let capabilities = self::unit(world, unit)?.radio_capabilities();
    ensure!(
        !mode.digital || capabilities.digital,
        "Your radio can't handle digital frequencies!"
    );
    ensure!(
        !mode.info || capabilities.info,
        "This unit is unable to use info functionality."
    );
    ensure!(
        !mode.scan || capabilities.scan,
        "This unit is unable to scan."
    );
    ensure!(
        !mode.relay || capabilities.relay,
        "This unit is unable to relay."
    );
    storage(world, unit)?.radio[usize::from(channel)].mode = mode;
    Ok(())
}

/// Shared letter/equal-sign parsing for cockpit radio settings.
pub(super) fn selection(input: &str) -> Result<(u8, &str)> {
    let (channel, value) = input.trim_start().split_once('=').context("Missing =!")?;
    let channel = channel.trim().as_bytes();
    ensure!(
        channel.len() == 1 && channel[0].is_ascii_alphabetic(),
        "Invalid channel-letter!"
    );
    Ok((channel[0].to_ascii_uppercase() - b'A', value.trim_start()))
}

/// Native configuration and listing use the same saved channel model as the domain API.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<String> {
        let mut world = ctx.scripts.world.borrow_mut();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::power::controlled(&world, unit, ctx.player)?;
        if matches!(input.name.as_str(), "listfreqs" | "listchannels") {
            let mut lines = vec!["# -- Mode -- Frequency -- Comtitle".to_string()];
            for (i, ch) in self::unit(&world, unit)?
                .radio_channels()
                .iter()
                .enumerate()
            {
                lines.push(format!(
                    "{}    {}{}{}{}    {:<9}    {}",
                    char::from(b'A' + i as u8),
                    if ch.mode.digital { 'D' } else { 'A' },
                    if ch.mode.relay { 'R' } else { '-' },
                    if ch.mode.muted { 'M' } else { '-' },
                    if ch.mode.scan {
                        'S'
                    } else {
                        ch.mode
                            .color
                            .unwrap_or(if ch.mode.info { 'I' } else { '-' })
                    },
                    ch.frequency,
                    ch.title
                ));
            }
            return Ok(lines.join("\n"));
        }
        let (channel, value) = selection(&input.args)?;
        let letter = char::from(b'A' + channel);
        match input.name.as_str() {
            "setchannelfreq" => {
                let frequency = parse_frequency(value)?;
                drop(world);
                super::set_radio_frequency_action(
                    ctx.scripts,
                    ctx.config,
                    unit,
                    ctx.player,
                    channel,
                    frequency,
                )?;
                Ok(format!("Channel {letter} set to {frequency}."))
            }
            "setchanneltitle" => {
                set_radio_title(&mut world, unit, ctx.player, channel, value)?;
                Ok(if value.is_empty() {
                    format!("Channel {letter} title cleared.")
                } else {
                    format!("Channel {letter} title set to set to {value}.")
                })
            }
            _ => {
                let mode = RadioMode::parse(value, self::unit(&world, unit)?.radio_capabilities())?;
                set_radio_mode(&mut world, unit, ctx.player, channel, mode.clone())?;
                if value.is_empty() {
                    return Ok(format!("Channel {letter} <send> mode set to analog."));
                }
                let mut flags = String::new();
                if mode.info {
                    flags.push('I');
                }
                if mode.muted {
                    flags.push('U');
                }
                if mode.relay {
                    flags.push('E');
                }
                if mode.scan {
                    flags.push('S');
                }
                if flags.is_empty() {
                    flags.push('-');
                }
                if let Some(color) = mode.color {
                    flags.push_str(&format!("/color:{color}"));
                }
                Ok(format!(
                    "Channel {letter} <send> mode set to {} (flags:{flags}).",
                    if mode.digital { "digital" } else { "analog" }
                ))
            }
        }
    })();
    Ok(crate::CommandAction::Report(crate::CommandReport::Reply(
        result.unwrap_or_else(|error| format!("{error:#}")),
    )))
}

impl super::Vehicle {
    /// Installed radio hardware uses the common equipment rules.
    pub fn radio_capabilities(&self) -> RadioCapabilities {
        capabilities(
            &self.definition().attributes,
            self.definition().has_special("Clan"),
            self.hardware,
        )
    }

    /// Active channel settings in letter order.
    pub fn radio_channels(&self) -> &[RadioChannel] {
        &self.radio[..usize::from(self.radio_capabilities().channels)]
    }

    /// Communication target sampled at startup.
    pub fn radio_skill(&self) -> i16 {
        self.radio_skill
    }

    /// Seconds until another interfered reception can attempt experience.
    pub fn radio_experience_remaining(&self) -> u8 {
        self.radio_experience_remaining
    }
}

/// Borrow the facts radio rules need without depending on chassis anatomy.
pub(super) struct RadioUnit<'a> {
    state: super::scanner::ScannerUnit<'a>,
    channels: &'a [RadioChannel],
    capabilities: RadioCapabilities,
    skill: i16,
    stun: bool,
}

impl RadioUnit<'_> {
    pub(super) fn radio_channels(&self) -> &[RadioChannel] {
        self.channels
    }
    pub(super) fn radio_capabilities(&self) -> RadioCapabilities {
        self.capabilities
    }
    pub(super) fn radio_skill(&self) -> i16 {
        self.skill
    }
    pub(super) fn is_destroyed(&self) -> bool {
        self.state.destroyed
    }
    pub(super) fn is_observer(&self) -> bool {
        self.state.observer
    }
    pub(super) fn power(&self) -> super::Power {
        self.state.power
    }
    pub(super) fn position(&self) -> Option<super::Position> {
        self.state.position
    }
    pub(super) fn signature(&self) -> super::UnitSignature {
        self.state.signature
    }
    pub(super) fn battlefield_id(&self) -> Option<String> {
        self.state.label()
    }
    pub(super) fn stunned(&self) -> bool {
        self.stun
    }
}

/// Project a Mech or vehicle onto the shared radio interface.
pub(super) fn unit(world: &World, id: ObjectId) -> Result<RadioUnit<'_>> {
    let state = super::scanner::scanner_unit(world, id).context("Unit is not constructed")?;
    let (channels, capabilities, skill, stun) = if let Some(unit) = world.btech.vehicles().get(&id)
    {
        (
            unit.radio_channels(),
            unit.radio_capabilities(),
            unit.radio_skill(),
            unit.crew_stunned(),
        )
    } else {
        let unit = &world.btech.constructed_units()[&id];
        (
            unit.radio_channels(),
            unit.radio_capabilities(),
            unit.radio_skill(),
            unit.stun_remaining() > 0,
        )
    };
    Ok(RadioUnit {
        state,
        channels,
        capabilities,
        skill,
        stun,
    })
}

/// Disjoint mutable storage used by common scanning and reception rules.
pub(super) struct RadioStorage<'a> {
    pub(super) radio: &'a mut [RadioChannel; 16],
    pub(super) dice: &'a mut super::Dice,
    pub(super) remaining: &'a mut u8,
    pub(super) pilot: Option<ObjectId>,
}

/// Borrow only radio-owned state and the unit's existing random stream.
pub(super) fn storage(world: &mut World, id: ObjectId) -> Result<RadioStorage<'_>> {
    if world.btech.vehicles().contains_key(&id) {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let pilot = unit.pilot();
        return Ok(RadioStorage {
            radio: &mut unit.radio,
            dice: &mut unit.dice,
            remaining: &mut unit.radio_experience_remaining,
            pilot,
        });
    }
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit is not constructed")?;
    let pilot = unit.pilot();
    Ok(RadioStorage {
        radio: &mut unit.radio,
        dice: &mut unit.dice,
        remaining: &mut unit.radio_experience_remaining,
        pilot,
    })
}

/// Common template radio capabilities for native and scripting controls.
pub fn unit_radio_capabilities(world: &World, id: ObjectId) -> Result<RadioCapabilities> {
    Ok(unit(world, id)?.radio_capabilities())
}

/// Snapshot the operator's communication skill at startup for all chassis.
pub(super) fn startup_skill(world: &World, pilot: Option<ObjectId>) -> i16 {
    pilot
        .filter(|pilot| {
            world
                .objects
                .get(pilot)
                .is_some_and(|object| object.kind == crate::Kind::Player)
        })
        .map_or(default_skill(), |pilot| {
            super::skills::character_skill_target(
                world,
                pilot,
                "Comm-Conventional",
                super::SkillCategory::Mental,
            )
            .expect("validated player")
        })
}

/// Reject malformed hardware ratings before either chassis becomes live.
pub(super) fn validate_attributes(
    attributes: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    for (field, value) in attributes {
        match field.as_str() {
            "radio_range" => ensure!(
                value.parse::<u16>().is_ok_and(|value| value <= 32767),
                "Invalid radio range"
            ),
            "radiotype" => ensure!(value.parse::<u8>().is_ok(), "Invalid radio configuration"),
            "radio" => ensure!(
                value.parse::<u8>().is_ok_and(|value| value <= 5),
                "Invalid radio quality"
            ),
            _ => {}
        }
    }
    Ok(())
}
