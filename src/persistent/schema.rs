// @generated automatically by Diesel CLI.

diesel::table! {
    contests (id) {
        id -> Integer,
        contest_name -> Text,
        contest_from -> Timestamp,
        contest_to -> Timestamp,
        problem_ids -> Text,
        user_ids -> Text,
        submission_limit -> Integer,
    }
}

diesel::table! {
    jobs (id) {
        id -> Integer,
        created_time -> Timestamp,
        updated_time -> Timestamp,
        source_code -> Text,
        lang -> Text,
        user_id -> Integer,
        contest_id -> Integer,
        problem_id -> Integer,
        job_state -> Integer,
        result -> Integer,
        score -> Double,
        cases -> Text,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        user_role -> Integer,
        user_name -> Text,
        passwd -> Text,
    }
}

diesel::joinable!(jobs -> contests (contest_id));
diesel::joinable!(jobs -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(contests, jobs, users,);
