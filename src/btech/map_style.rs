//! Shared map palette expressed as trusted document styles for capability-aware client rendering.
use super::Terrain;

/// Terrain colors shared by tactical and long-range displays; grassland keeps the default style.
pub(super) fn terrain(terrain: Terrain, elevation: u8) -> &'static str {
    match terrain {
        Terrain::HighWater => "[fg=blue]",
        Terrain::Water if elevation < 2 => "[fg=blue bold]",
        Terrain::Water => "[fg=blue]",
        Terrain::Building | Terrain::Ice | Terrain::Wall | Terrain::Snow => "[fg=white bold]",
        Terrain::Road | Terrain::Smoke => "[fg=black bold]",
        Terrain::Rough => "[fg=yellow bold]",
        Terrain::Mountains => "[fg=yellow]",
        Terrain::Fire => "[fg=red bold]",
        Terrain::LightForest => "[fg=green bold]",
        Terrain::HeavyForest => "[fg=green]",
        Terrain::Bridge => "",
        Terrain::Grassland => "",
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
