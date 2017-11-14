-- When creating a table using diesel's migration, ideally we'd use this pragma
-- command, but we receive the error "cannot change into wal mode from within a
-- transaction" instead.
-- PRAGMA journal_mode=WAL;
CREATE TABLE logs(
    ri INTEGER PRIMARY KEY NOT NULL,
    epoch INT8 NOT NULL,
    remote_addr TEXT,
    remote_user TEXT,
    status INT,
    method TEXT,
    path TEXT,
    version TEXT,
    body_bytes_sent INT,
    referer TEXT,
    user_agent TEXT,
    host TEXT NOT NULL
);

CREATE index idx_epoch on logs(epoch);
CREATE index idx_host ON logs(host);
