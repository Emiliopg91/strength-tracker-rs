--- Covering index for per-session set/record flags

CREATE INDEX SET_SESSION_PR ON "SET"(session, pr);
