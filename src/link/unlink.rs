
use poise::Modal;

use crate::{Context, Error};


/// Unlinks your discord account to your roblox's
#[poise::command(slash_command, prefix_command)]
pub async fn unlink(
    ctx: Context<'_>
) -> Result<(), Error> {
    let user_id = ctx.author().id.0 as i64;
    let conn = &ctx.data().0;

    if sqlx::query!("SELECT * FROM users WHERE discord_id=$1", user_id).fetch_one(conn).await.is_err() {
        ctx.say("Your discord account is not linked to any roblox account").await?;
        return Ok(());
    }

    let Context::Application(app_ctx) = ctx else {
        return Ok(())
    };

    #[derive(Debug, poise::Modal)]
    #[name = "Are you sure you want to unlink ?"]
    struct ConfirmationModal {
        #[name = "Confirmation"]
        #[placeholder = "Yes"]
        #[min_length = 2]
        #[max_length = 3]
        confirmation: Option<String>,
    }

    let res = ConfirmationModal::execute(app_ctx).await?;

    let Some(res) = res else {
        ctx.say("You didn't anwser the question, the unlink process was aborted").await?;
        return Ok(());
    };

    let Some(res) = res.confirmation else {
        ctx.say("You didn't anwser the question, the unlink process was aborted").await?;
        return Ok(());
    };
    
    if res.to_lowercase() != "yes" {
        ctx.say("You haven't anwsered yes, the unlink process was aborted").await?;
    }

    if sqlx::query!("DELETE FROM users WHERE discord_id=$1", user_id).execute(conn).await.is_err() {
        ctx.say("Could not unlink your accounts").await?;
    } else {
        ctx.say("Successfully unlinked your discord account and roblox account").await?;
    }

    Ok(())
}