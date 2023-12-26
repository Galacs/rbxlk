use poise::serenity_prelude::{self as serenity, ActionRowComponent};
use poise::event::Event;

use crate::{Data, Error, link::link::link_backend};


pub async fn event_handler(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        Event::Ready { data_about_bot } => {
            println!("Logged in as {}", data_about_bot.user.name);
        }
        Event::InteractionCreate { interaction } => {
            let Some(interaction) = interaction.clone().message_component() else {
                return Ok(())
            };
            println!("Ok");
            if interaction.data.custom_id == "link" {
                interaction.create_interaction_response(&ctx.http, |r| {
                    r.kind(serenity::InteractionResponseType::Modal)
                    .interaction_response_data(|d| {
                        d.content("Linking process")
                        .custom_id("roblox_link")
                        .title("What is your roblox username ?")
                        .components(|c| {
                            c.create_action_row(|r| r.create_input_text(|i| {
                                i.custom_id("roblox_username")
                                .placeholder("Roblox username")
                                .label("Roblox username")
                                .style(serenity::InputTextStyle::Short)
                                .max_length(20)
                                .required(true)
                            }))
                        })
                    })
                }).await?;
                let modal_response = &interaction.get_interaction_response(&ctx.http).await?;
                let interaction = match modal_response
                    .await_modal_interaction(ctx)
                    .timeout(std::time::Duration::from_secs(60 * 3))
                    .await
                {
                    Some(x) => x,
                    None => {
                        return Ok(());
                    }
                };
                let username: &ActionRowComponent = &interaction.data.components[0].components[0];
                let username = match username {
                    ActionRowComponent::InputText(txt) => &txt.value,
                    _ => return Ok(()),
                };

                link_backend(Some(std::env::var("ROBLOSECURITY").ok()), interaction.user.id.0 as i64, data, modal_response, &ctx.http, username).await?;
                interaction.defer(&ctx.http).await?;

            }
        }
        _ => {}
    }
    Ok(())
}