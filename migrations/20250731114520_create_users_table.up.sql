-- Add migration script here

CREATE TABLE "user" (
    id INTEGER PRIMARY KEY,
    email VARCHAR NOT NULL,
    name VARCHAR
);

CREATE INDEX "idx_user" ON "user" ("id");

INSERT INTO "user" ("id", "email") VALUES (123, 'john.doe@mail.com');