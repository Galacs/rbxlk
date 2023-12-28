use actix_web::{post, web, App, HttpServer, Responder, HttpRequest, HttpResponse, http::{header::ContentType, StatusCode}};
use anyhow::Error;
use sqlx::{Pool, Postgres, PgPool};

pub struct Data(Pool<Postgres>, Option<String>, Option<String>);

fn parse_human_readable_number(input: &str) -> Option<f64> {
    let multiplier = match input.chars().last()? {
        'K' | 'k' => 1_000.0,
        'M' | 'm' => 1_000_000.0,
        'B' | 'b' => 1_000_000_000.0,
        _ => return input.parse().ok(),
    };

    let num_str = &input[..input.len() - 1];
    num_str.parse::<f64>().ok().map(|num| num * multiplier)
}

#[post("/give")]
async fn give(req: HttpRequest, data: web::Data<Data>) -> impl Responder {
    if let Some(secret) = &data.as_ref().2 {
        let Some(header) = req.headers().get("security") else {
            return HttpResponse::Unauthorized().body("Error: no security header provided");
        };
        if header.to_str().unwrap() != secret {
            return HttpResponse::Unauthorized().body("Error: wrong security header");
        }
    }

    let conn = &data.as_ref().0;

    let Some(username) = req.headers().get("username") else {
        return HttpResponse::BadRequest().body("Error: no username header provided");
    };
    let mut username = username.to_str().unwrap().replace('@', "");
    let Some(amount) = req.headers().get("amount") else {
        return HttpResponse::build(StatusCode::BAD_REQUEST)
            .insert_header(ContentType::html())
            .body("Error: no amount header provided");
    };
    // let Ok(amount) = amount.to_str().unwrap().parse::<i64>() else {
    let Some(amount) = parse_human_readable_number(amount.to_str().unwrap()) else {
        return HttpResponse::BadRequest().body("Amount malformed");
    };

    async fn get_closest_user(pool: &Pool<Postgres>, username: &mut String) -> Result<i64, Error> {
        let Ok(user) = sqlx::query!("SELECT * FROM users ORDER BY SIMILARITY(roblox_username, $1) DESC LIMIT 1", *username).fetch_one(pool).await else {
            return Err(anyhow::anyhow!("User not found error"))
        };
        if let Some(name) = user.roblox_username {
            *username = name;
        }
        Ok(user.roblox_id)
    }

    let Ok(roblox_id) = get_closest_user(conn, &mut username).await else {
        return HttpResponse::BadRequest().body("User not found");
    };

    let Ok(row) = sqlx::query!("UPDATE users SET balance = balance + $1 WHERE roblox_id = $2", amount as i64, roblox_id).execute(conn).await else {
        return HttpResponse::InternalServerError().body("Database error");
    };

    if row.rows_affected() < 1 {
        return HttpResponse::BadRequest().body("User not found in db");
    }

    HttpResponse::Ok().body(format!("{} was given {}", username, amount))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // DB
    let database_url = std::env::var("DATABASE_URL").expect("Expected a database url in the environment");
    let conn = PgPool::connect(&database_url).await?;
    sqlx::migrate!("../../migrations").run(&conn).await?;

    let security = std::env::var("SECURITY_HEADER").ok();
    if security.is_none() {
        println!("Unsafe: No security header set")
    }

    HttpServer::new(move || {
        App::new().service(give).app_data(web::Data::new(Data(conn.clone(), std::env::var("ROBLOSECURITY").ok(), security.to_owned())))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await?;
    Ok(())
}