//! Native catalogue form inspection uses the shared stock-name index and paced report delivery.
use anyhow::Result;

/// LISTFORMS ignores its argument and leaves inventory and world state unchanged.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = super::part_forms(&ctx.scripts.world(), ctx.player).map(|forms| {
        let mut lines = vec!["Listing of forms:".to_owned()];
        for (index, form) in forms.iter().enumerate() {
            lines.push(format!(
                "{index:3} {:<20} {:<25} {}",
                form.short_name, form.long_name, form.very_long_name
            ));
        }
        lines.join("\n")
    });
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Inspection(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}
