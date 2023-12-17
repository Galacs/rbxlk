# rbxlk

## How to run it

### .env file

You have to create a .env file containing optionaly a GUILD_ID line, a DISCORD_TOKEN and a ROBLOSECURITY
```
DATABASE_URL="postgres://rbxlk:mypassword@localhost/rbxlk"
GUILD_ID=id
DISCORD_TOKEN=token
ROBLOSECURITY=cookie
```

### Using docker compose

You can the included docker compose file to directly spin up the bot and its services with a postgres database.

#### Run sqlx prepare in the root, services/cron and services/web

```bash
cargo sqlx prepare
```

#### Build and start all containers

```bash
docker compose up -d
```

## Using the web api

To add or remove balance to users, you can send a post request to /give on port 8080, with a username header and an amount header.