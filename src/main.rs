use poise::{serenity_prelude::{self as serenity, User, CacheHttp, UserId, CreateInteractionResponse, CreateInteractionResponseFollowup, Member, GuildId, RoleId}, ApplicationCommandOrAutocompleteInteraction};
use roboat::{catalog::Item, RoboatError};
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
    async fn create_interaction_followup<'a, F>(
        &self,
        http: impl AsRef<serenity::Http> + std::marker::Send,
        f: F,
    ) -> Result<(), Error>
    where
        for<'b> F: FnOnce(
            &'b mut CreateInteractionResponseFollowup<'a>,
        ) -> &'b mut CreateInteractionResponseFollowup<'a> + std::marker::Send;
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
    async fn create_interaction_followup<'a, F>(
        &self,
        http: impl AsRef<serenity::Http> + std::marker::Send,
        f: F,
    ) -> Result<(), Error>
    where
        for<'b> F: FnOnce(
            &'b mut CreateInteractionResponseFollowup<'a>,
        ) -> &'b mut CreateInteractionResponseFollowup<'a> + std::marker::Send
    {
        match self {
            MergedInteraction::SerenityApplicationInteraction(interaction) => interaction.create_followup_message(http, f).await?,
            MergedInteraction::SerenityMessageComponentInteraction(interaction) => interaction.create_followup_message(http, f).await?,
            MergedInteraction::SerenityModalSubmitInteraction(interaction) => interaction.create_followup_message(http, f).await?,
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
    complete_backend(&ctx.author(), &ctx.guild_id().unwrap(), ctx.data(), &merged_interaction, ctx.http()).await?;
    Ok(())
}

async fn complete_backend(author_id: &User, guild_id: &GuildId, data: &Data, ctx: &impl crate::InteractionReponse, http: &impl CacheHttp) -> Result<(), Error> {
    let conn = &data.0;
    let client = &data.1;
    let user_id = author_id.id.0 as i64;

    let Ok(row) = sqlx::query!("SELECT roblox_id,string FROM verif WHERE discord_id=$1", user_id).fetch_one(conn).await else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("You have no ongoing verification process, to start one, use the /link command"))).await?;
        return Ok(())
    };

    let user = client.user_details(row.roblox_id as u64).await?;
    let role = sqlx::query!("SELECT role_id from role").fetch_one(&data.0).await?.role_id.unwrap_or(0);
    let mut member = guild_id.member(&http, author_id.id).await?;
    member.add_role(&http.http(), RoleId(role as u64)).await?;

    if user.description.contains(&row.string) {
        let mut tx = conn.begin().await?;
        sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(&mut *tx).await?;
        let Ok(_) = sqlx::query!("INSERT INTO users(discord_id, roblox_id, roblox_username) VALUES ($1,$2,$3)", user_id, row.roblox_id as i64, user.username).execute(&mut *tx).await else {
            ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("This roblox account ({}) is already linked with another discord account", row.roblox_id)).ephemeral(true))).await?;
            return Ok(());
        };
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
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You don't have enough balance to withdraw {amount} robux, to do that, you need {price} and you only have {}", row.balance)).ephemeral(true))).await?;
    }

    if sqlx::query!("INSERT INTO withdraw(discord_id, amount, price) VALUES ($1, $2, $3)", author_id, amount, price).execute(conn).await.is_err() {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("Your already have a withdrawal process with that amount").ephemeral(true))).await?;
        return Ok(())
    }

    ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| {
        m.content(format!("Withdrawal process of {} robux for a price of {} initiated for <@{}> with roblox account {}, Please create an asset worth that amount", amount, price, author_id, row.roblox_username.unwrap_or_default()))
        .ephemeral(true)
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

pub fn get_id_from_usr_input(mut input: String) -> Result<u64, Error> {
    input = input.replace("://", "");
    Ok({if input.contains('/') {
        input.split('/').nth(2).unwrap_or_default()
    } else {
        &input
    }}.parse()?)
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
    
    let rate = sqlx::query!("SELECT rate from swap_rate").fetch_one(&data.0).await?.rate;
    let price = (amount as f32 * rate).floor() as i32;

    let Ok(row) = sqlx::query!("SELECT roblox_id,withdraw.discord_id,users.balance FROM withdraw JOIN users ON withdraw.discord_id = users.discord_id WHERE withdraw.discord_id=$1 AND amount=$2", user_id, amount).fetch_one(conn).await else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content("You have no ongoing withdrawal process, to start one, use the corresponding embed").ephemeral(true))).await?;
        return Ok(())
    };

    dbg!(item_id);
    let future = tokio::time::timeout(tokio::time::Duration::from_secs(2), client.item_details(vec![Item { item_type: roboat::catalog::ItemType::Asset, id: item_id }]));

    if let Ok(item) = dbg!(future.await)? {
        let item = item.get(0).ok_or("no item result")?;
        let item_price = item.price.ok_or("no price")?;
        let product_id = item.product_id.ok_or("no product id")?;
        if item_price > amount as u64 {
            ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Your item is too expensive, it should be {} robux but it is {} robux", amount, item_price)).ephemeral(true))).await?;
        }
        if price as i64 > row.balance {
            ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("You don't have enough balance on your account, you need {} but you only have {}", price, row.balance)).ephemeral(true))).await?;
        }
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Trying to buy {} for {} robux", item.name, amount)).ephemeral(true))).await?;
        let _ = dbg!(client.purchase_tradable_limited(product_id, item.creator_id, 0, price as u64).await);
    } else {
        ctx.create_interaction_response(http.http(), |i| i.interaction_response_data(|m| m.content(format!("Trying to buy game pass item for {} robux", amount)).ephemeral(true))).await?;
        if let Err(err) = dbg!(client.purchase_tradable_limited(item_id, row.roblox_id as u64, 0, amount as u64).await) {
            if let RoboatError::PurchaseTradableLimitedError(roboat::PurchaseTradableLimitedError::PriceChanged(p)) = err {
                ctx.create_interaction_followup(http.http(), |m| m.content(format!("Your item price was set wrong, it should cost {} robux but it costs {}", amount, p)).ephemeral(true)).await?;
                return Ok(());
            }
            ctx.create_interaction_followup(http.http(), |m| m.content("There was an error while trying to buy your item").ephemeral(true)).await?;
            return Ok(());
        };
    }

    sqlx::query!("UPDATE users SET balance = balance - $1 WHERE discord_id = $2", price as i64, author_id.0 as i64).execute(conn).await?;
    ctx.create_interaction_followup(http.http(), |i| i.content(format!("You just withdrawed {} robux and used {} credit", amount, price)).ephemeral(true)).await?;
    sqlx::query!("DELETE FROM withdraw WHERE discord_id=$1 AND amount=$2", user_id, amount).execute(conn).await?;

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

/// Sets global linked role
#[poise::command(slash_command, prefix_command, owners_only, hide_in_help)]
async fn set_role(
    ctx: Context<'_>,
    role: serenity::Role,
) -> Result<(), Error> {
    let conn = &ctx.data().0;
    let role_id = role.id.0 as i64;
    sqlx::query!("UPDATE role SET role_id = $1", role_id).execute(conn).await?;
    ctx.say(format!("Role is now set to {}", role.name)).await?;
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

    let owner_id = {
        let env_var = std::env::var("OWNER_ID");
        if let Ok(str) = env_var {
            UserId(str.parse().unwrap_or_default())
        } else {
            UserId(0)           
        }
    };
    let mut owners = std::collections::HashSet::<serenity::UserId>::new();
    owners.insert(owner_id);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![balance(), set_rate(), set_role(), link(), unlink(), create_link_embed(), create_withdraw_embed(), complete(), cancel()],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event::event_handler(ctx, event, framework, data))
            },
            owners,
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