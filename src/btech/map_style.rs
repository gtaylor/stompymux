//! Shared map palette and height digits for the tactical and long-range displays.
use super::{Hex, Terrain};

/// The height digit maps print for a hex: the depth of open water or ice, otherwise the height
/// of its top surface, so a building or bridge shows its roof or deck.
pub(super) fn shown_height(hex: Hex) -> u8 {
    if hex.is_water_surface() {
        return hex.water_depth();
    }
    u8::try_from(hex.top_height()).unwrap_or(u8::MAX)
}

/// Colors shared by tactical and long-range displays; clear ground keeps the default style.
/// Shallow water is brighter than deep water, and lighter foliage brighter than denser.
pub(super) fn terrain(hex: Hex) -> &'static str {
    match hex.terrain() {
        Terrain::Water if hex.water_depth() < 2 => "[fg=blue bold]",
        Terrain::Water => "[fg=blue]",
        Terrain::Building
        | Terrain::Wall
        | Terrain::Ice
        | Terrain::ThinSnow
        | Terrain::DeepSnow => "[fg=white bold]",
        Terrain::Road | Terrain::Pavement | Terrain::Rail | Terrain::Smoke => "[fg=black bold]",
        Terrain::Rough | Terrain::Rubble | Terrain::Sand => "[fg=yellow bold]",
        Terrain::UltraRough | Terrain::UltraRubble | Terrain::Mud | Terrain::Tundra => {
            "[fg=yellow]"
        }
        Terrain::Fire | Terrain::Magma => "[fg=red bold]",
        Terrain::MagmaCrust => "[fg=red]",
        Terrain::HeavyIndustrial => "[fg=magenta]",
        Terrain::LightWoods | Terrain::LightJungle | Terrain::PlantedFields => "[fg=green bold]",
        Terrain::HeavyWoods
        | Terrain::UltraHeavyWoods
        | Terrain::HeavyJungle
        | Terrain::UltraHeavyJungle => "[fg=green]",
        Terrain::Swamp => "[fg=cyan]",
        Terrain::Bridge | Terrain::Clear => "",
    }
}

/// Own-unit emphasis and relative affiliation colors for acquired contacts.
pub(super) fn contact(own: bool, friendly: bool) -> &'static str {
    if own {
        return "[bold]";
    }
    if friendly {
        "[fg=yellow bold]"
    } else {
        "[fg=red bold]"
    }
}
