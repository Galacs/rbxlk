use poise::serenity_prelude::{self as serenity, User, CacheHttp};
use sqlx::{Pool, Postgres, PgPool};
use async_trait::async_trait;

mod link;
mod event;

use link::{create_link_embed::create_embed, link::link, unlink::unlink};

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
    let conn = &ctx.data().0;
    let client = &ctx.data().1;
    let user_id = ctx.author().id.0 as i64;
    let Ok(row) = sqlx::query!("SELECT roblox_id,string FROM verif WHERE discord_id=$1", user_id).fetch_one(conn).await else {
        ctx.say("You have no ongoing verification process, to start one, use the /link command").await?;
        return Ok(())
    };

    let user = client.user_details(row.roblox_id as u64).await?;

    if user.description.contains(&row.string) {
        let mut tx = conn.begin().await?;
        sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO users(discord_id, roblox_id) VALUES ($1,$2)", user_id, row.roblox_id as i64).execute(&mut *tx).await?;
        tx.commit().await?;
        ctx.say(format!("Your discord account was successfully linked with {}, id: {}", user.username, user.id)).await?;
    } else {
        ctx.say(format!("Your specified roblox account, {}, id: {}, doesn't currently have the string {} inside its description", user.username, user.id, row.string)).await?;
    }

    Ok(())
}

/// Cancels a verification attempt
#[poise::command(slash_command, prefix_command)]
async fn cancel(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let conn = &ctx.data().0;
    let user_id = ctx.author().id.0 as i64;

    sqlx::query!("DELETE FROM verif WHERE discord_id=$1", user_id).execute(conn).await?;
    ctx.say("Your verification attempt was cancelled, use /link to start a new one").await?;
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
            commands: vec![balance(), link(), unlink(), create_embed(), complete(), cancel()],
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