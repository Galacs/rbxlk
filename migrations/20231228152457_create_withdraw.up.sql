CREATE TABLE withdraw (
    discord_id BIGINT NOT NULL,
    amount INTEGER NOT NULL,
    price INTEGER NOT NULL,
    start_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (discord_id, amount)
)