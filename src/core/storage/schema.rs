// @generated automatically by Diesel CLI.

diesel::table! {
    clusters (id) {
        id -> Integer,
        cluster_name -> Text,
        scheduler -> Integer,
        max_jobs -> Nullable<Integer>,
    }
}

diesel::table! {
    configs (id) {
        id -> Integer,
        config_name -> Text,
        cluster_id -> Integer,
        flags -> Json,
        env -> Json,
    }
}

diesel::table! {
    jobs (id) {
        id -> Integer,
        job_name -> Text,
        config_id -> Integer,
        submit_time -> Integer,
        directory -> Text,
        command -> Text,
        status -> Text,
        job_id -> Nullable<Text>,
        end_time -> Nullable<Integer>,
        preprocess -> Nullable<Text>,
        postprocess -> Nullable<Text>,
        archived -> Nullable<Integer>,
        variables -> Json,
    }
}

diesel::table! {
    virtualqueue (id) {
        id -> Integer,
        job_id -> Integer,
    }
}

diesel::joinable!(configs -> clusters (cluster_id));
diesel::joinable!(jobs -> configs (config_id));
diesel::joinable!(virtualqueue -> jobs (job_id));

diesel::allow_tables_to_appear_in_same_query!(clusters, configs, jobs, virtualqueue,);
