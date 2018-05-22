table! {
    logs (ri) {
        ri -> Nullable<Integer>,
        epoch -> BigInt,
        remote_addr -> Nullable<Text>,
        remote_user -> Nullable<Text>,
        status -> Nullable<Integer>,
        method -> Nullable<Text>,
        path -> Nullable<Text>,
        version -> Nullable<Text>,
        body_bytes_sent -> Nullable<Integer>,
        referer -> Nullable<Text>,
        user_agent -> Nullable<Text>,
        host -> Text,
    }
}
