CREATE TABLE users (
    discord_id BIGINT PRIMARY KEY NOT NULL UNIQUE,
    roblox_id BIGINT NOT NULL UNIQUE,
    balance BIGINT NOT NULL DEFAULT 0
)