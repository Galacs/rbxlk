
use poise::serenity_prelude::CacheHttp;

use crate::{Context, Error, Data};

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;


pub async fn link_backend(robosecurity: Option<Option<String>>, author_id: i64, data: &Data, ctx: &impl crate::Repliable, http: &impl CacheHttp, input: &str) -> Result<(), Error> {
    let conn = &data.0;
    let mut client = &data.1;
    let a: roboat::Client;
    if let Some(opt) = robosecurity {
        a = roboat::ClientBuilder::new().roblosecurity(opt.unwrap_or_default()).build();
        client = &a;
    }

    let user_id = author_id;



    if let Ok(row) = sqlx::query!("SELECT * FROM users WHERE discord_id=$1", user_id).fetch_one(conn).await {
        let user = client.user_details(row.roblox_id as u64).await?;
        println!("after");

        ctx.send_reply(&http, format!("You discord account is already linked to {}, id: {}", user.username, user.id)).await?;

        return Ok(());
    }

    if let Ok(row) = sqlx::query!("SELECT * FROM verif WHERE discord_id=$1", user_id).fetch_one(conn).await {
        let user = client.user_details(row.roblox_id as u64).await?;
        ctx.send_reply(http, format!("You already have started a verification process with the Roblox username {}, id: {}, to cancel that one, use /cancel", user.username, user.id)).await?;
        return Ok(());
    }

    let (roblox_id, roblox_display_name) = match input.parse::<u64>() {
        Err(_) => {
            let users = vec![input.to_owned()];
            let all_username_user_details = client.username_user_details(users, true).await?;
            let user = all_username_user_details.first().ok_or("User not found")?;
            (user.id, user.display_name.clone())
        },
        Ok(id) => {
            let user = client.user_details(id).await?;
            (user.id, user.display_name)
        },
    };

    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();

    sqlx::query!("INSERT INTO verif(discord_id, roblox_id, string) VALUES ($1, $2, $3)", user_id, roblox_id as i64, &rand_string).execute(conn).await?;

    ctx.send_reply(http, format!("Found user Roblox user {} with the id {}.\nVerification has started, please put the string {} in your profile's description and use /complete to complete the verification", roblox_display_name, roblox_id, rand_string)).await?;


    Ok(())
}

/// Links your discord account to your roblox's
#[poise::command(slash_command, prefix_command)]
pub async fn link(
    ctx: Context<'_>,
    #[description = "Roblox username or id"] input: String,
) -> Result<(), Error> {


    link_backend(None, ctx.author().id.0 as i64, ctx.data(), &ctx, ctx.http(), &input).await?;

    ctx.defer().await?;

    Ok(())
}