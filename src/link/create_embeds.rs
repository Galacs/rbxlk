
use crate::{Context, Error};


/// Creates an embed message for linking
#[poise::command(slash_command, prefix_command, owners_only, hide_in_help)]
pub async fn create_link_embed(
    ctx: Context<'_>,
) -> Result<(), Error> {
     ctx.send(|b| {
        b.ephemeral(true)
        .content(format!("The embed was sent in channel <#{}>", ctx.channel_id().0))
    }).await?;
    
    ctx.channel_id().send_message(&ctx.http(), |m| {
        m.add_embed(|e| {
            e.title("Link")
            .description("Click this button to start the linking process")
        }).components(|c| {
            c.create_action_row(|r| r.create_button(|b| {
                b.label("Start linking")
                .custom_id("link")
            }))
        })
    }).await?;
    Ok(())
}


/// Creates an embed message for withdrawing
#[poise::command(slash_command, prefix_command, owners_only, hide_in_help)]
pub async fn create_withdraw_embed(
    ctx: Context<'_>,
) -> Result<(), Error> {
     ctx.send(|b| {
        b.ephemeral(true)
        .content(format!("The embed was sent in channel <#{}>", ctx.channel_id().0))
    }).await?;
    
    ctx.channel_id().send_message(&ctx.http(), |m| {
        m.add_embed(|e| {
            e.title("Withdrawing")
            .description("Click this button to start the withdraw process")
        }).components(|c| {
            c.create_action_row(|r| r.create_button(|b| {
                b.label("Start withdrawing")
                .custom_id("withdraw")
            }))
        })
    }).await?;
    Ok(())
}