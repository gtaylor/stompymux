//! Explicit saves participate in the host's existing commit and publication transaction.
use crate::{ObjectId, Scripts};
use anyhow::{Result, ensure};

/// Stage a wizard checkpoint and its confirmation; only the host performs persistence.
/// Callback or output failure removes both the request and its pending confirmation.
pub fn request_database_save(scripts: &Scripts, actor: ObjectId) -> Result<()> {
    scripts.atomic(|_| {
        ensure!(
            crate::authority::is_wizard(&scripts.world(), actor),
            "Permission denied."
        );
        super::notify_message(
            scripts,
            super::MessageTarget::Player(actor),
            "SQLite checkpoint complete.",
        )?;
        scripts.effects.request_save();
        scripts.effects.validate()?;
        Ok(())
    })
}

/// SAVEDB ignores trailing input and commits through the ordinary serialized world owner.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    Ok(match request_database_save(ctx.scripts, ctx.player) {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
