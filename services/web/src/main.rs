use actix_web::{post, web, App, HttpServer, Responder, HttpRequest, HttpResponse, http::{header::ContentType, StatusCode}};
use sqlx::{Pool, Postgres, PgPool};

pub struct Data(Pool<Postgres>, Option<String>);

#[post("/give")]
async fn greet(req: HttpRequest, data: web::Data<Data>) -> impl Responder {
    let conn = &data.as_ref().0;
    let roblosecurity = &data.as_ref().1;

    let Some(username) = req.headers().get("username") else {
        return HttpResponse::BadRequest().body("Error: no username header provided");
    };
    let username = username.to_str().unwrap();
    let Some(amount) = req.headers().get("amount") else {
        return HttpResponse::build(StatusCode::BAD_REQUEST)
            .insert_header(ContentType::html())
            .body("Error: no amount header provided");
    };
    let Ok(amount) = amount.to_str().unwrap().parse::<i64>() else {
        return HttpResponse::BadRequest().body("Amount malformed");
    };

    // Roblox API client
    let client = roboat::ClientBuilder::new()
        .roblosecurity(roblosecurity.clone().unwrap_or_default())
        .build();

    let users = vec![username.to_owned()];
    let Ok(all_username_user_details) = client.username_user_details(users, true).await else {
        return HttpResponse::BadRequest().body("User not found");
    };
    let Ok(user) = all_username_user_details.first().ok_or("User not found") else {
        return HttpResponse::BadRequest().body("User not found");
    };
    let roblox_id = user.id as i64;

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



    HttpServer::new(move || {
        App::new().service(greet).app_data(web::Data::new(Data(conn.clone(), std::env::var("ROBLOSECURITY").ok())))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await?;
    Ok(())
}