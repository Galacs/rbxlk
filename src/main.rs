use poise::{serenity_prelude::{self as serenity, User, CacheHttp, UserId, CreateInteractionResponse}, ApplicationCommandOrAutocompleteInteraction};
use roboat::catalog::Item;
use sqlx::{Pool, Postgres, PgPool};
use async_trait::async_trait;

mod link;
mod event;

use link::{create_embeds::{create_link_embed, create_withdraw_embed}, link::link, unlink::unlink};

pub struct Data(Pool<Postgres>, roboat::Client);
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

// Returns false if user isn't in the db
async fn user_exists(user_id: i64, conn: &Pool<Postgres>) -> Result<bool, Error> {
    if sqlx::query!("SELECT * FROM users WHERE discord_id = $1", user_id).fetch_one(conn).await.is_err() {
        Ok(false)
    } else {
        Ok(true)
    }
}

#[async_trait]
trait Repliable {
    async fn send_reply(&self, http: &impl CacheHttp, msg: String) -> Result<(), Error>;
}

#[async_trait]
impl Repliable for Context<'_> {
    async fn send_reply(&self, _http: &impl CacheHttp, msg: String) -> Result<(), Error> {
        self.say(msg).await?;
        Ok(())
    }
}

#[async_trait]
impl Repliable for serenity::channel::Message {
    async fn send_reply(&self, http: &impl CacheHttp, msg: String) -> Result<(), Error> {
        self.reply(http, msg).await?;
        Ok(())
    }
}

pub enum MergedInteraction<'a> {
    SerenityApplicationInteraction(&'a serenity::ApplicationCommandInteraction),
    SerenityMessageComponentInteraction(serenity::MessageComponentInteraction),
    SerenityModalSubmitInteraction(serenity::ModalSubmitInteraction),
}

#[async_trait]
pub trait InteractionReponse {
    async fn create_interaction_response<'a, F>(
        &self,
        http: impl AsRef<serenity::Http> + std::marker::Send,
        f: F,
    ) -> Result<(), Error>
    where
        for<'b> F: FnOnce(
            &'b mut CreateInteractionResponse<'a>,
        ) -> &'b mut CreateInteractionResponse<'a> + std::marker::Send;
}

#[async_trait]
impl InteractionReponse for MergedInteraction<'_> {
    async fn create_interaction_response<'a, F>(
        &self,
        http: impl AsRef<serenity::Http> + std::marker::Send,
        f: F,
    ) -> Result<(), Error>
    where
        for<'b> F: FnOnce(
            &'b mut CreateInteractionResponse<'a>,
        ) -> &'b mut CreateInteractionResponse<'a> + std::marker::Send
    {
        match self {
            MergedInteraction::SerenityApplicationInteraction(interaction) => interaction.create_interaction_response(http, f).await?,
            MergedInteraction::SerenityMessageComponentInteraction(interaction) => interaction.create_interaction_response(http, f).await?,
            MergedInteraction::SerenityModalSubmitInteraction(interaction) => interaction.create_interaction_response(http, f).await?,
        };
        Ok(())
    }
}

fn get_pretty_username(user: &User) -> String {
    match user.discriminator {
        0000 => user.name.clone(),
        _ => user.tag(),
    }
}
/// Displays your or another user's account balance
#[poise::command(slash_command, prefix_command)]
async fn balance(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let conn = &ctx.data().0;
    let user_id = ctx.author().id.0 as i64;

    if !user_exists(user_id, conn).await? {
        match &user {
            Some(u) => ctx.say(format!("{} hasn't linked their accounts", get_pretty_username(u))).await?,
            None => ctx.say("You haven't linked your accounts").await?,
        };
        return Ok(())
    }

    let balance = sqlx::query!("SELECT balance FROM users WHERE discord_id=$1", user_id).fetch_one(conn).await?.balance;

    match &user {
        Some(u) => ctx.say(format!("{} has {}", get_pretty_username(u), balance)).await?,
        None => ctx.say(format!("You have {}", balance)).await?,
    };

    Ok(())
}

/// Complete the link of your discord account to your roblox's
#[poise::command(slash_command, prefix_command)]
async fn complete(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let Context::Application(app_ctx) = ctx else {
        return Ok(())
    };
    let ApplicationCommandOrAutocompleteInteraction::ApplicationCommand(interaction) = app_ctx.interaction else {
        return Ok(())
    };
    let merged_interaction = MergedInteraction::SerenityApplicationInteraction(interaction);
    complete_backend(&ctx.author().id, ctx.data(), &merged_interaction, ctx.http()).await?;
    Ok(())
}

async fn complete_backend(author_id: &UserId, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp) -> Result<(), Error> {
    let conn = &data.0;
    let client = &data.1;
    let user_id = author_id.0 as i64;

    let Ok(row) = sqlx::query!("SELECT roblox_id,string FROM verif WHERE discord_id=$1", user_id).fetch_one(conn).await else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("You have no ongoing verification process, to start one, use the /link command"))).await?;
        return Ok(())
    };

    let user = client.user_details(row.roblox_id as u64).await?;

    if user.description.contains(&row.string) {
        let mut tx = conn.begin().await?;
        sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO users(discord_id, roblox_id, roblox_username) VALUES ($1,$2,$3)", user_id, row.roblox_id as i64, user.username).execute(&mut *tx).await?;
        tx.commit().await?;
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Your discord account was successfully linked with {}, id: {}", user.username, user.id)))).await?;
    } else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Your specified roblox account, {}, id: {}, doesn't currently have the string {} inside its description", user.username, user.id, row.string)))).await?;
    }
    Ok(())
}

pub async fn withdraw_backend(author_id: i64, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp, amount: i32) -> Result<(), Error> {
    let conn = &data.0;
    let rate = sqlx::query!("SELECT rate from swap_rate").fetch_one(&data.0).await?.rate;
    let price = (amount as f32 * rate).floor() as i32;

    let Ok(row) = sqlx::query!("SELECT balance,roblox_username FROM users WHERE discord_id=$1", author_id).fetch_one(conn).await else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("You haven't linked your accounts"))).await?;
        return Ok(())
    };

    if row.balance < price as i64 {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You don't have enough balance to withdraw {amount} robux, to do that, you need {price} and you only have {}", row.balance)))).await?;
    }

    if sqlx::query!("INSERT INTO withdraw(discord_id, amount, price) VALUES ($1, $2, $3)", author_id, amount, price).execute(conn).await.is_err() {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("Your already have a withdrawal process with that amount"))).await?;
        return Ok(())
    }

    ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| {
        m.content(format!("Withdrawal process of {} robux for a price of {} initiated for <@{}> with roblox account {}, Please create an asset worth that amount", amount, price, author_id, row.roblox_username.unwrap_or_default()))
        .components(|c| c.create_action_row(|r| {
            r.create_button(|b| {
                b.custom_id(format!("withdraw_complete-{}", amount))
                .label("Continue")
                .style(serenity::ButtonStyle::Success)
            })
            .create_button(|b| {
                b.custom_id(format!("withdraw_cancel-{}", amount))
                .label("Cancel")
                .style(serenity::ButtonStyle::Danger)
            })
        }))   
    })).await?;
    Ok(())
}


/// Cancels a verification attempt
#[poise::command(slash_command, prefix_command)]
async fn cancel(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let Context::Application(app_ctx) = ctx else {
        return Ok(())
    };
    let ApplicationCommandOrAutocompleteInteraction::ApplicationCommand(interaction) = app_ctx.interaction else {
        return Ok(())
    };
    let merged_interaction = MergedInteraction::SerenityApplicationInteraction(interaction);
    cancel_backend(&ctx.author().id, ctx.data(), &merged_interaction, ctx.http()).await?;
    Ok(())
}

async fn cancel_backend(author_id: &UserId, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp) -> Result<(), Error> {
    let conn = &data.0;
    let user_id = author_id.0 as i64;

    sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(conn).await?;
    ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("Your verification attempt was cancelled, use /link to start a new one"))).await?;
    Ok(())
}

async fn cancel_withdraw_backend(author_id: &UserId, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp, amount: i32) -> Result<(), Error> {
    let conn = &data.0;
    let user_id = author_id.0 as i64;
    sqlx::query!("DELETE FROM withdraw WHERE discord_id=$1 AND amount=$2", user_id, amount).execute(conn).await?;
    ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You withdrawal process for {} robux was cancelled", amount)))).await?;
    Ok(())
}

async fn complete_withdraw_backend(author_id: &UserId, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp, amount: i32, item_id: u64) -> Result<(), Error> {
    let conn = &data.0;
    let client = &data.1;
    let user_id = author_id.0 as i64;

    let Ok(row) = sqlx::query!("SELECT roblox_id,withdraw.discord_id FROM withdraw JOIN users ON withdraw.discord_id = users.discord_id WHERE withdraw.discord_id=$1 AND amount=$2", user_id, amount).fetch_one(conn).await else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("You have no ongoing verification process, to start one, use the /link command"))).await?;
        return Ok(())
    };

    let all_details = dbg!(client.item_details(vec![Item { item_type: roboat::catalog::ItemType::Asset, id: item_id }]).await)?;

    let collectible_item_id = dbg!(client.collectible_item_id(item_id).await)?;

    let collectible_product_id = dbg!(client.collectible_product_id(collectible_item_id.clone()).await?);

    let collectible_creator_id = dbg!(client.collectible_creator_id(collectible_item_id.clone()).await?);

    client.purchase_non_tradable_limited(
        collectible_item_id,
        collectible_product_id,
        collectible_creator_id,
        10,
    ).await?;

    println!("Purchased item {} for {} robux", item_id, 10);

    // let user = client.user_details(row.roblox_id as u64).await?;
    // if user.description.contains(&row.string) {
    //     let mut tx = conn.begin().await?;
    //     sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(&mut *tx).await?;
    //     sqlx::query!("INSERT INTO users(discord_id, roblox_id, roblox_username) VALUES ($1,$2,$3)", user_id, row.roblox_id as i64, user.username).execute(&mut *tx).await?;
    //     tx.commit().await?;
    //     ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Your discord account was successfully linked with {}, id: {}", user.username, user.id)))).await?;
    // } else {
    //     ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Your specified roblox account, {}, id: {}, doesn't currently have the string {} inside its description", user.username, user.id, row.string)))).await?;
    // }
    Ok(())
}

/// Sets global swap rate
#[poise::command(slash_command, prefix_command, owners_only, hide_in_help)]
async fn set_rate(
    ctx: Context<'_>,
    rate: f32,
) -> Result<(), Error> {
    let conn = &ctx.data().0;
    sqlx::query!("UPDATE swap_rate SET rate = $1", rate).execute(conn).await?;
    ctx.say(format!("Rate is now set to {}", rate)).await?;
    Ok(())
}



#[tokio::main]
async fn main() -> Result<(), Error> {
    // Loads dotenv file
    let _ = dotenv::dotenv();

    // DB
    let database_url = std::env::var("DATABASE_URL").expect("Expected a database url in the environment");
    let conn = PgPool::connect(&database_url).await?;
    sqlx::migrate!().run(&conn).await?;

    // Roblox API client
    let client = roboat::ClientBuilder::new()
        .roblosecurity(std::env::var("ROBLOSECURITY").unwrap_or_default())
        .build();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![balance(), set_rate(), link(), unlink(), create_link_embed(), create_withdraw_embed(), complete(), cancel()],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event::event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .token(std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"))
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                if let Ok(var) = std::env::var("GUILD_ID") {
                    poise::builtins::register_in_guild(ctx, &framework.options().commands, serenity::GuildId(var.parse().expect("GUILD_ID should be an integer"))).await?;
                }
                else {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                }
                Ok(Data(conn, client))
            })
        });

    framework.run().await?;
    Ok(())
}