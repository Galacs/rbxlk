use poise::serenity_prelude::{self as serenity, ActionRowComponent};
use poise::event::Event;

use crate::{MergedInteraction, complete_backend, cancel_backend, cancel_withdraw_backend, withdraw_backend, complete_withdraw_backend, get_id_from_usr_input};
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
            let merged_interaction = MergedInteraction::SerenityMessageComponentInteraction(interaction.clone());
            match interaction.data.custom_id.as_str() {
                "link" => {
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
                    let merged_interaction = MergedInteraction::SerenityModalSubmitInteraction(interaction.as_ref().clone());
                    let username: &ActionRowComponent = &interaction.data.components[0].components[0];
                    let username = match username {
                        ActionRowComponent::InputText(txt) => &txt.value,
                        _ => return Ok(()),
                    };
                    link_backend(interaction.user.id.0 as i64, data, &merged_interaction, &ctx.http, username).await?;
        
                },
                "complete" => {
                    complete_backend(&interaction.user, &interaction.guild_id.unwrap(), data, &merged_interaction, &ctx.http).await?;
                },
                "cancel" => {
                    cancel_backend(&interaction.user.id, data, &merged_interaction, &ctx.http).await?;
                },
                "withdraw" => {
                    let rate = sqlx::query!("SELECT rate from swap_rate").fetch_one(&data.0).await?.rate;
                    interaction.create_interaction_response(&ctx.http, |r| {
                        r.kind(serenity::InteractionResponseType::Modal)
                        .interaction_response_data(|d| {
                            d.content("Withdrawal process")
                            .custom_id("roblox_link")
                            .title("How many robux do you want to withdraw ?")
                            .components(|c| {
                                c.create_action_row(|r| r.create_input_text(|i| {
                                    i.custom_id("amount")
                                    .placeholder(format!("Current rate: {}", rate))
                                    .label("Robux amount")
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
                    let merged_interaction = MergedInteraction::SerenityModalSubmitInteraction(interaction.as_ref().clone());
                    let amount: &ActionRowComponent = &interaction.data.components[0].components[0];
                    let amount: i32 = match amount {
                        ActionRowComponent::InputText(txt) => txt.value.parse()?,
                        _ => return Ok(()),
                    };
                    withdraw_backend(interaction.user.id.0 as i64, data, &merged_interaction, &ctx.http, amount).await?;
                },
                _ => {
                    if interaction.data.custom_id.starts_with("withdraw_cancel") {
                        let amount = interaction.data.custom_id.split('-').nth(1).ok_or("no amount in custom_id")?.parse()?;
                        cancel_withdraw_backend(&interaction.user.id, data, &merged_interaction, &ctx.http, amount).await?;
                    } else if interaction.data.custom_id.starts_with("withdraw_complete") {
                        let amount = interaction.data.custom_id.split('-').nth(1).ok_or("no amount in custom_id")?.parse()?;
                        interaction.create_interaction_response(&ctx.http, |r| {
                            r.kind(serenity::InteractionResponseType::Modal)
                            .interaction_response_data(|d| {
                                d.content("Withdrawal process")
                                .custom_id("item_id")
                                .title("What is the id/product id/url of the item you created ?")
                                .components(|c| {
                                    c.create_action_row(|r| r.create_input_text(|i| {
                                        i.custom_id("id")
                                        .label("Item ID")
                                        .style(serenity::InputTextStyle::Short)
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
                        let merged_interaction = MergedInteraction::SerenityModalSubmitInteraction(interaction.as_ref().clone());
                        let item_id: &ActionRowComponent = &interaction.data.components[0].components[0];
                        let item_id = match item_id {
                            ActionRowComponent::InputText(txt) => txt.value.clone(),
                            _ => return Ok(()),
                        };
                        let item_id = get_id_from_usr_input(item_id)?;
                        complete_withdraw_backend(&interaction.user.id, data, &merged_interaction, &ctx.http, amount, item_id).await?;
                    }
                }
            }
            
        }
        _ => {}
    }
    Ok(())
}