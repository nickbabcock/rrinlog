CREATE TABLE logs(
    epoch INT8 PRIMARY KEY NOT NULL,
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

CREATE index idx_host ON logs(host)
