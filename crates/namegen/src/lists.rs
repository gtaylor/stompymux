//! Word lists: the embedded defaults, optional overrides from a directory, and the
//! validation that keeps every generator call infallible once the lists have loaded.
use crate::patterns::{Pattern, Slot};
use crate::{Culture, Flavor};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every embedded list file by its path relative to the lists directory.
const FILES: &[(&str, &str)] = &[
    ("outfit.toml", include_str!("../data/outfit.toml")),
    ("callsigns.toml", include_str!("../data/callsigns.toml")),
    (
        "people/anglo.toml",
        include_str!("../data/people/anglo.toml"),
    ),
    (
        "people/hispanic.toml",
        include_str!("../data/people/hispanic.toml"),
    ),
    (
        "people/east_asian.toml",
        include_str!("../data/people/east_asian.toml"),
    ),
    (
        "people/slavic.toml",
        include_str!("../data/people/slavic.toml"),
    ),
    (
        "people/germanic.toml",
        include_str!("../data/people/germanic.toml"),
    ),
    (
        "people/arabic.toml",
        include_str!("../data/people/arabic.toml"),
    ),
    (
        "people/south_asian.toml",
        include_str!("../data/people/south_asian.toml"),
    ),
    (
        "people/west_african.toml",
        include_str!("../data/people/west_african.toml"),
    ),
];

/// A word list with a shared core and optional additions per flavor.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlavoredList {
    #[serde(default)]
    pub common: Vec<String>,
    #[serde(default)]
    pub house: Vec<String>,
    #[serde(default)]
    pub periphery: Vec<String>,
    #[serde(default)]
    pub pirate: Vec<String>,
}

impl FlavoredList {
    /// The flavor-only additions.
    fn extra(&self, flavor: Flavor) -> &[String] {
        match flavor {
            Flavor::House => &self.house,
            Flavor::Periphery => &self.periphery,
            Flavor::Pirate => &self.pirate,
        }
    }

    /// The words available to one flavor: the common core plus that flavor's additions.
    pub fn for_flavor(&self, flavor: Flavor) -> Vec<&str> {
        self.common
            .iter()
            .chain(self.extra(flavor))
            .map(String::as_str)
            .collect()
    }

    fn all(&self) -> impl Iterator<Item = &String> {
        self.common
            .iter()
            .chain(&self.house)
            .chain(&self.periphery)
            .chain(&self.pirate)
    }
}

/// One outfit pattern as written in `outfit.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPattern {
    template: String,
    weight: u32,
    /// Flavors that may use this pattern; omitted means all of them.
    #[serde(default)]
    flavors: Vec<Flavor>,
}

/// A parsed outfit pattern with its selection weight.
#[derive(Debug, Clone)]
pub struct WeightedPattern {
    pub pattern: Pattern,
    pub weight: u32,
    flavors: Vec<Flavor>,
}

impl WeightedPattern {
    /// Whether `flavor` may use this pattern.
    pub fn allows(&self, flavor: Flavor) -> bool {
        self.flavors.is_empty() || self.flavors.contains(&flavor)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutfitLists {
    patterns: Vec<RawPattern>,
    /// Case-insensitive fragments that must not appear in a generated outfit name.
    #[serde(default)]
    blocked: Vec<String>,
    #[serde(default)]
    adjectives: FlavoredList,
    #[serde(default)]
    nouns: FlavoredList,
    #[serde(default)]
    suffixes: FlavoredList,
    #[serde(default)]
    colors: FlavoredList,
    #[serde(default)]
    places: FlavoredList,
}

/// Everything outfit names draw from.
#[derive(Debug, Clone)]
pub struct OutfitLists {
    pub patterns: Vec<WeightedPattern>,
    pub blocked: Vec<String>,
    pub adjectives: FlavoredList,
    pub nouns: FlavoredList,
    pub suffixes: FlavoredList,
    pub colors: FlavoredList,
    pub places: FlavoredList,
}

impl OutfitLists {
    /// The word list behind a slot, or `None` for derived slots.
    pub fn list(&self, slot: Slot) -> Option<&FlavoredList> {
        Some(match slot {
            Slot::Adjective => &self.adjectives,
            Slot::Noun => &self.nouns,
            Slot::Suffix => &self.suffixes,
            Slot::Color => &self.colors,
            Slot::Place => &self.places,
            Slot::Ordinal | Slot::Possessive => return None,
        })
    }
}

/// Callsign material: the words themselves and occasional prefixes such as "Old".
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallsignLists {
    pub words: Vec<String>,
    #[serde(default)]
    pub prefixes: Vec<String>,
}

/// Given and family names for one culture.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeopleLists {
    pub given: Vec<String>,
    pub family: Vec<String>,
}

/// All word lists, validated so generation cannot fail.
#[derive(Debug, Clone)]
pub struct Lists {
    pub outfit: OutfitLists,
    pub callsigns: CallsignLists,
    pub people: BTreeMap<Culture, PeopleLists>,
}

impl Lists {
    /// The lists compiled into the crate.
    pub fn embedded() -> Result<Self> {
        Self::load(|_| Ok(None))
    }

    /// The embedded lists, with any file present under `dir` replacing its embedded
    /// counterpart. Files are matched by relative path, for example `people/anglo.toml`.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        Self::load(|relative| {
            let path = dir.join(relative);
            if !path.is_file() {
                return Ok(None);
            }
            std::fs::read_to_string(&path)
                .map(Some)
                .with_context(|| format!("Reading {}", path.display()))
        })
    }

    fn load(mut override_for: impl FnMut(&str) -> Result<Option<String>>) -> Result<Self> {
        let mut source = |relative: &str| -> Result<String> {
            if let Some(text) = override_for(relative)? {
                return Ok(text);
            }
            FILES
                .iter()
                .find(|(path, _)| *path == relative)
                .map(|(_, text)| (*text).to_string())
                .with_context(|| format!("No embedded list {relative}"))
        };
        let outfit: RawOutfitLists =
            toml::from_str(&source("outfit.toml")?).context("Parsing outfit.toml")?;
        let callsigns: CallsignLists =
            toml::from_str(&source("callsigns.toml")?).context("Parsing callsigns.toml")?;
        let mut people = BTreeMap::new();
        for culture in Culture::ALL {
            let relative = format!("people/{}.toml", culture.name());
            let lists: PeopleLists = toml::from_str(&source(&relative)?)
                .with_context(|| format!("Parsing {relative}"))?;
            people.insert(*culture, lists);
        }

        let patterns = outfit
            .patterns
            .iter()
            .map(|raw| {
                Ok(WeightedPattern {
                    pattern: Pattern::parse(&raw.template)?,
                    weight: raw.weight,
                    flavors: raw.flavors.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let lists = Self {
            outfit: OutfitLists {
                patterns,
                blocked: outfit.blocked,
                adjectives: outfit.adjectives,
                nouns: outfit.nouns,
                suffixes: outfit.suffixes,
                colors: outfit.colors,
                places: outfit.places,
            },
            callsigns,
            people,
        };
        lists.validate()?;
        Ok(lists)
    }

    /// Reject lists that would make some generation path impossible or produce odd output.
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.outfit.patterns.is_empty(),
            "outfit.toml has no patterns"
        );
        for flavor in Flavor::ALL {
            ensure!(
                self.outfit
                    .patterns
                    .iter()
                    .any(|pattern| pattern.allows(*flavor) && pattern.weight > 0),
                "No outfit pattern with positive weight allows the {} flavor",
                flavor.name()
            );
            for pattern in &self.outfit.patterns {
                if !pattern.allows(*flavor) {
                    continue;
                }
                for piece in pattern.pattern.pieces() {
                    let crate::patterns::Piece::Slot(slot) = piece else {
                        continue;
                    };
                    if let Some(list) = self.outfit.list(*slot) {
                        ensure!(
                            !list.for_flavor(*flavor).is_empty(),
                            "Pattern {:?} needs {:?} words but the {} flavor has none",
                            pattern.pattern.template,
                            slot,
                            flavor.name()
                        );
                    }
                }
            }
        }
        for (name, list) in [
            ("adjectives", &self.outfit.adjectives),
            ("nouns", &self.outfit.nouns),
            ("suffixes", &self.outfit.suffixes),
            ("colors", &self.outfit.colors),
            ("places", &self.outfit.places),
        ] {
            check_words(name, list.all())?;
        }
        check_words("blocked", self.outfit.blocked.iter())?;
        ensure!(
            !self.callsigns.words.is_empty(),
            "callsigns.toml has no words"
        );
        check_words("callsign words", self.callsigns.words.iter())?;
        check_words("callsign prefixes", self.callsigns.prefixes.iter())?;
        for (culture, lists) in &self.people {
            ensure!(
                !lists.given.is_empty() && !lists.family.is_empty(),
                "people/{}.toml needs both given and family names",
                culture.name()
            );
            check_words(
                &format!("{} given names", culture.name()),
                lists.given.iter(),
            )?;
            check_words(
                &format!("{} family names", culture.name()),
                lists.family.iter(),
            )?;
        }
        Ok(())
    }
}

/// Words must be non-blank, free of surrounding whitespace, and unique within their list.
fn check_words<'a>(list: &str, words: impl Iterator<Item = &'a String>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for word in words {
        if word.trim().is_empty() {
            bail!("The {list} list has a blank entry");
        }
        if word.trim() != word {
            bail!("The {list} entry {word:?} has surrounding whitespace");
        }
        if !seen.insert(word.to_lowercase()) {
            bail!("The {list} list repeats {word:?}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_lists_load() {
        let lists = Lists::embedded().unwrap();
        assert_eq!(lists.people.len(), Culture::ALL.len());
        assert!(lists.outfit.patterns.len() >= 5);
    }

    #[test]
    fn overrides_replace_only_the_files_present() {
        let dir = std::env::temp_dir().join(format!("namegen-lists-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("callsigns.toml"),
            "words = [\"Solo\"]\nprefixes = []\n",
        )
        .unwrap();
        let lists = Lists::from_dir(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(lists.callsigns.words, ["Solo"]);
        assert!(!lists.outfit.nouns.common.is_empty());
    }

    #[test]
    fn duplicate_words_are_rejected() {
        let dir = std::env::temp_dir().join(format!("namegen-dupes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("callsigns.toml"), "words = [\"Solo\", \"solo\"]\n").unwrap();
        let error = Lists::from_dir(&dir).unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(error.to_string().contains("repeats"), "{error}");
    }
}
