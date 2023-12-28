
use poise::{serenity_prelude, ApplicationCommandOrAutocompleteInteraction};
use poise::serenity_prelude::CacheHttp;

use crate::{Context, Error, Data, MergedInteraction};

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;


pub async fn link_backend(author_id: i64, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp, input: &str) -> Result<(), Error> {
    let conn = &data.0;
    let client = &data.1;
    let user_id = author_id;

    if let Ok(row) = sqlx::query!("SELECT * FROM users WHERE discord_id=$1", user_id).fetch_one(conn).await {
        let user = client.user_details(row.roblox_id as u64).await?;
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You discord account is already linked to {}, id: {}", user.username, user.id)).ephemeral(true))).await?;
        return Ok(());
    }

    if let Ok(row) = sqlx::query!("SELECT * FROM verif WHERE discord_id=$1", user_id).fetch_one(conn).await {
        let user = client.user_details(row.roblox_id as u64).await?;
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You already have started a verification process with the Roblox username {}, id: {}, to cancel that one, use /cancel", user.username, user.id)).ephemeral(true))).await?;
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

    ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| {
        m.content(format!("Found user Roblox user {} with the id {}.\nVerification has started, please put the string {} in your profile's description and use /complete to complete the verification or press the confirm button", roblox_display_name, roblox_id, rand_string))
        .ephemeral(true)
        .components(|c| c.create_action_row(|r| {
            r.create_button(|b| {
                b.custom_id("complete")
                .label("Confirm")
                .style(serenity_prelude::ButtonStyle::Success)
            })
            .create_button(|b| {
                b.custom_id("cancel")
                .label("Cancel")
                .style(serenity_prelude::ButtonStyle::Danger)
            })
        }))   
    })).await?;
    Ok(())
}

/// Links your discord account to your roblox's
#[poise::command(slash_command, prefix_command)]
pub async fn link(
    ctx: Context<'_>,
    #[description = "Roblox username or id"] input: String,
) -> Result<(), Error> {
    let Context::Application(app_ctx) = ctx else {
        return Ok(())
    };
    let ApplicationCommandOrAutocompleteInteraction::ApplicationCommand(interaction) = app_ctx.interaction else {
        return Ok(())
    };
    let merged_interaction = MergedInteraction::SerenityApplicationInteraction(interaction);

    link_backend(ctx.author().id.0 as i64, ctx.data(), &merged_interaction, ctx.http(), &input).await?;

    Ok(())
}