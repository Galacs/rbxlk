use sqlx::PgPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Loads dotenv file
    let _ = dotenv::dotenv();

    // DB
    let database_url = std::env::var("DATABASE_URL").expect("Expected a database url in the environment");
    let conn = PgPool::connect(&database_url).await?;
    sqlx::migrate!("../../migrations").run(&conn).await?;

    // Cleans verif table of attempts older than 3 days
    let query = sqlx::query!("DELETE FROM verif WHERE start_date < CURRENT_TIMESTAMP - INTERVAL '3 days'").execute(&conn).await?;

    println!("Deleted {} old verification records", query.rows_affected());

    Ok(())
}