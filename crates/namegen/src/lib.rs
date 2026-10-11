//! Seeded name generation for mercenary outfits, the people in them, and their callsigns.
//!
//! Every name is a pure function of a [`NameSpec`]: the same seed, culture and flavor always
//! produce the same text. Word lists are embedded in the crate and can be overridden from a
//! directory with [`Lists::from_dir`]. Generation never fails once the lists have loaded.
//!
//! ```
//! use stompymux_namegen::{Lists, NameSpec};
//!
//! let lists = Lists::embedded().unwrap();
//! let spec = NameSpec { seed: Some(42), ..NameSpec::default() };
//! let outfit = lists.outfit_name(&spec);
//! assert_eq!(outfit, lists.outfit_name(&spec));
//! ```
mod lists;
pub mod patterns;

pub use lists::{CallsignLists, FlavoredList, Lists, OutfitLists, PeopleLists, WeightedPattern};

use patterns::{Slot, ordinal, possessive};
use serde::{Deserialize, Serialize};
use stompymux_seeded_rng::{Rng, mix, random_seed};

/// Naming tradition for people. Drawn once per outfit so a roster reads as coherent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Culture {
    Anglo,
    Hispanic,
    EastAsian,
    Slavic,
    Germanic,
    Arabic,
    SouthAsian,
    WestAfrican,
}

impl Culture {
    /// Every culture, in the order used for seeded selection.
    pub const ALL: &[Self] = &[
        Self::Anglo,
        Self::Hispanic,
        Self::EastAsian,
        Self::Slavic,
        Self::Germanic,
        Self::Arabic,
        Self::SouthAsian,
        Self::WestAfrican,
    ];

    /// The snake_case name used in files and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Anglo => "anglo",
            Self::Hispanic => "hispanic",
            Self::EastAsian => "east_asian",
            Self::Slavic => "slavic",
            Self::Germanic => "germanic",
            Self::Arabic => "arabic",
            Self::SouthAsian => "south_asian",
            Self::WestAfrican => "west_african",
        }
    }
}

/// The kind of employer an outfit grew up around, which steers word choice and patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    /// Former house regulars: numbered regiments, places, formal unit types.
    House,
    /// Frontier militia turned professional: homespun, rugged, place-bound.
    Periphery,
    /// Raiders and bandits: menacing, boastful, a commander's name up front.
    Pirate,
}

impl Flavor {
    /// Every flavor, in the order used for seeded selection.
    pub const ALL: &[Self] = &[Self::House, Self::Periphery, Self::Pirate];

    /// The snake_case name used in files and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::House => "house",
            Self::Periphery => "periphery",
            Self::Pirate => "pirate",
        }
    }
}

/// What to generate. Every field is optional; [`NameSpec::resolve`] fills the rest from the
/// seed so the resolved spec reproduces the names exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameSpec {
    /// Random seed. Omit for a seed from the clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Naming tradition for people and possessive outfit names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub culture: Option<Culture>,
    /// Outfit naming style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<Flavor>,
}

/// A spec with every choice made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSpec {
    pub seed: u64,
    pub culture: Culture,
    pub flavor: Flavor,
}

impl NameSpec {
    /// Fill in the seed, culture and flavor. Culture and flavor come from their own streams,
    /// so fixing one by hand does not change the other.
    pub fn resolve(&self) -> ResolvedSpec {
        let seed = self.seed.unwrap_or_else(random_seed);
        let culture = self
            .culture
            .unwrap_or_else(|| *Rng::stream(seed, "culture").pick(Culture::ALL));
        let flavor = self
            .flavor
            .unwrap_or_else(|| *Rng::stream(seed, "flavor").pick(Flavor::ALL));
        ResolvedSpec {
            seed,
            culture,
            flavor,
        }
    }

    /// `count` specs for a batch. The first keeps this spec's seed, so a batch of one is the
    /// single name; the rest derive their seeds from it.
    pub fn series(&self, count: usize) -> Vec<Self> {
        let base = self.seed.unwrap_or_else(random_seed);
        (0..count as u64)
            .map(|index| Self {
                seed: Some(if index == 0 { base } else { mix(base ^ index) }),
                ..*self
            })
            .collect()
    }
}

impl From<ResolvedSpec> for NameSpec {
    fn from(resolved: ResolvedSpec) -> Self {
        Self {
            seed: Some(resolved.seed),
            culture: Some(resolved.culture),
            flavor: Some(resolved.flavor),
        }
    }
}

/// A generated outfit name and how it was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutfitName {
    pub text: String,
    /// The pattern template the name was built from.
    pub pattern: String,
    pub spec: ResolvedSpec,
}

/// A generated person's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonName {
    pub given: String,
    pub family: String,
    pub culture: Culture,
}

impl PersonName {
    /// Given name then family name.
    pub fn full(&self) -> String {
        format!("{} {}", self.given, self.family)
    }
}

impl std::fmt::Display for PersonName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.given, self.family)
    }
}

impl std::fmt::Display for OutfitName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// How many times a generator rerolls a blocked or repeated result before accepting it.
const REROLLS: usize = 16;

/// Probability that a callsign carries a prefix such as "Old".
const CALLSIGN_PREFIX_CHANCE: f64 = 0.12;

impl Lists {
    /// A mercenary outfit name.
    pub fn outfit_name(&self, spec: &NameSpec) -> OutfitName {
        let resolved = spec.resolve();
        let mut rng = Rng::stream(resolved.seed, "outfit");
        let candidates: Vec<&WeightedPattern> = self
            .outfit
            .patterns
            .iter()
            .filter(|pattern| pattern.allows(resolved.flavor))
            .collect();
        let people = &self.people[&resolved.culture];
        let mut last = None;
        for _ in 0..=REROLLS {
            let chosen = rng.pick_weighted(&candidates, |pattern| pattern.weight);
            let text = chosen.pattern.render(|slot| match slot {
                Slot::Ordinal => ordinal(if rng.chance(0.6) {
                    rng.between(1, 12) as u32
                } else {
                    rng.between(13, 99) as u32
                }),
                Slot::Possessive => possessive(rng.pick(&people.family)),
                _ => {
                    let list = self.outfit.list(slot).expect("word slots have lists");
                    (*rng.pick(&list.for_flavor(resolved.flavor))).to_string()
                }
            });
            let blocked = self.is_blocked(&text);
            last = Some(OutfitName {
                text,
                pattern: chosen.pattern.template.clone(),
                spec: resolved,
            });
            if !blocked {
                break;
            }
        }
        last.expect("at least one candidate is rendered")
    }

    /// Whether a name contains a blocked fragment, ignoring case.
    fn is_blocked(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.outfit
            .blocked
            .iter()
            .any(|fragment| lower.contains(&fragment.to_lowercase()))
    }

    /// One person's name in the spec's culture.
    pub fn person_name(&self, spec: &NameSpec) -> PersonName {
        let resolved = spec.resolve();
        let mut rng = Rng::stream(resolved.seed, "person");
        self.draw_person(&mut rng, resolved.culture)
    }

    /// `count` people in the spec's culture with distinct family names where the list
    /// allows it. Names do not overlap with [`Lists::person_name`] for the same spec, so a
    /// commander and their roster can share a seed.
    pub fn roster(&self, spec: &NameSpec, count: usize) -> Vec<PersonName> {
        let resolved = spec.resolve();
        let mut rng = Rng::stream(resolved.seed, "roster");
        let mut names: Vec<PersonName> = Vec::with_capacity(count);
        while names.len() < count {
            let mut candidate = self.draw_person(&mut rng, resolved.culture);
            for _ in 0..REROLLS {
                if !names.iter().any(|name| name.family == candidate.family) {
                    break;
                }
                candidate = self.draw_person(&mut rng, resolved.culture);
            }
            names.push(candidate);
        }
        names
    }

    fn draw_person(&self, rng: &mut Rng, culture: Culture) -> PersonName {
        let people = &self.people[&culture];
        PersonName {
            given: rng.pick(&people.given).clone(),
            family: rng.pick(&people.family).clone(),
            culture,
        }
    }

    /// One pilot callsign.
    pub fn callsign(&self, spec: &NameSpec) -> String {
        let resolved = spec.resolve();
        let mut rng = Rng::stream(resolved.seed, "callsign");
        self.draw_callsign(&mut rng)
    }

    /// `count` distinct callsigns where the list allows it.
    pub fn callsigns(&self, spec: &NameSpec, count: usize) -> Vec<String> {
        let resolved = spec.resolve();
        let mut rng = Rng::stream(resolved.seed, "callsigns");
        let mut names: Vec<String> = Vec::with_capacity(count);
        while names.len() < count {
            let mut candidate = self.draw_callsign(&mut rng);
            for _ in 0..REROLLS {
                if !names.contains(&candidate) {
                    break;
                }
                candidate = self.draw_callsign(&mut rng);
            }
            names.push(candidate);
        }
        names
    }

    fn draw_callsign(&self, rng: &mut Rng) -> String {
        let word = rng.pick(&self.callsigns.words);
        if !self.callsigns.prefixes.is_empty() && rng.chance(CALLSIGN_PREFIX_CHANCE) {
            format!("{} {word}", rng.pick(&self.callsigns.prefixes))
        } else {
            word.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn lists() -> Lists {
        Lists::embedded().unwrap()
    }

    fn spec(seed: u64) -> NameSpec {
        NameSpec {
            seed: Some(seed),
            ..NameSpec::default()
        }
    }

    #[test]
    fn names_are_reproducible() {
        let lists = lists();
        for seed in 0..50 {
            let spec = spec(seed);
            assert_eq!(lists.outfit_name(&spec), lists.outfit_name(&spec));
            assert_eq!(lists.person_name(&spec), lists.person_name(&spec));
            assert_eq!(lists.roster(&spec, 6), lists.roster(&spec, 6));
            assert_eq!(lists.callsign(&spec), lists.callsign(&spec));
        }
    }

    #[test]
    fn fixed_seeds_snapshot() {
        let lists = lists();
        let seen: Vec<String> = (1..=5)
            .map(|seed| {
                let spec = spec(seed);
                format!(
                    "{} | {} | {}",
                    lists.outfit_name(&spec),
                    lists.person_name(&spec),
                    lists.callsign(&spec)
                )
            })
            .collect();
        // Update these values deliberately: changing them changes every saved game's names.
        assert_eq!(
            seen,
            [
                "10th Nightfall Chasseurs | Baba Toure | Ace",
                "Redoubt Watch | Stanislav Tomic | Jackpot",
                "Sandoval's Jaegers | Dario Ibarra | Quotient",
                "Bykov's Carabiniers | Konstantin Zelenko | Nova",
                "Sokolov's Regiment | Pavel Zelenko | Beetle"
            ]
        );
    }

    #[test]
    fn fixing_culture_does_not_change_flavor_and_vice_versa() {
        let base = spec(9).resolve();
        let with_culture = NameSpec {
            culture: Some(Culture::Slavic),
            ..spec(9)
        }
        .resolve();
        let with_flavor = NameSpec {
            flavor: Some(Flavor::Pirate),
            ..spec(9)
        }
        .resolve();
        assert_eq!(with_culture.flavor, base.flavor);
        assert_eq!(with_flavor.culture, base.culture);
    }

    #[test]
    fn every_pattern_flavor_and_culture_appears() {
        let lists = lists();
        let mut patterns = BTreeSet::new();
        let mut flavors = BTreeSet::new();
        let mut cultures = BTreeSet::new();
        for seed in 0..2000 {
            let outfit = lists.outfit_name(&spec(seed));
            patterns.insert(outfit.pattern);
            flavors.insert(outfit.spec.flavor);
            cultures.insert(outfit.spec.culture);
        }
        assert_eq!(patterns.len(), lists.outfit.patterns.len(), "{patterns:?}");
        assert_eq!(flavors.len(), Flavor::ALL.len());
        assert_eq!(cultures.len(), Culture::ALL.len());
    }

    #[test]
    fn nothing_blocked_or_malformed_leaks_out() {
        let lists = lists();
        for seed in 0..3000 {
            let text = lists.outfit_name(&spec(seed)).text;
            assert!(!lists.is_blocked(&text), "{text}");
            assert!(!text.contains("  ") && text.trim() == text, "{text:?}");
            assert!(!text.contains('{') && !text.contains('}'), "{text:?}");
        }
    }

    #[test]
    fn rosters_avoid_repeated_family_names() {
        let lists = lists();
        for seed in 0..200 {
            let roster = lists.roster(&spec(seed), 8);
            let families: BTreeSet<_> = roster.iter().map(|name| &name.family).collect();
            assert_eq!(families.len(), 8, "{roster:?}");
            assert!(roster.iter().all(|name| name.culture == roster[0].culture));
        }
        let callsigns = lists.callsigns(&spec(1), 12);
        assert_eq!(callsigns.iter().collect::<BTreeSet<_>>().len(), 12);
    }

    #[test]
    fn series_keeps_the_first_seed() {
        let specs = spec(5).series(3);
        assert_eq!(specs[0], spec(5));
        assert_ne!(specs[1].seed, specs[2].seed);
        assert_ne!(specs[1].seed, Some(5));
    }
}
