use std::net::TcpListener;

use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    telemetry::{get_tracing_subscriber, init_tracing_subscriber},
};

//Ensure that the `tracing` stack is only initialised once using `once_cell`

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".into();
    let subscriber_name = "test".to_string();

    // We cannot assing the output oof `get_tracing_subscriber` to a variable based on the
    // value TEST_LOG because the sink is part of the type returned by
    // get_tracing_subscriber, therefore they are not the same type, we could
    // work around it, but this is the most straight-forward way of moving forward

    // let mut subscriber;
    // if std::env::var("TEST_LOG").is_ok() {
    //     subscriber = get_tracing_subscriber(default_filter_level, subscriber_name, std::io::stdout);
    // } else {
    //     subscriber = get_tracing_subscriber(default_filter_level, subscriber_name, std::io::sink);
    // };

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber =
            get_tracing_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_tracing_subscriber(subscriber);
    } else {
        let subscriber =
            get_tracing_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_tracing_subscriber(subscriber);
    }
});

#[tokio::test]
async fn health_check_works() {
    let TestApp { address, .. } = spawn_app().await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let TestApp { address, db_pool } = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40@gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(200, response.status().as_u16());

    let _saved = sqlx::query!("SELECT email, name from subscriptions",)
        .fetch_one(&db_pool)
        .await
        .expect("Failed to fetch saved subscription");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let TestApp { address, .. } = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad request when the payload was {}.",
            error_message
        );
    }
}

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

async fn spawn_app() -> TestApp {
    // The first time `initialize` is invoked the code `TRACING` is executed.
    // All other invocations will instead skip execution.
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");

    // we retrieve the port assigned to us by the OS
    let port = listener.local_addr().unwrap().port();
    let address = format!("http:127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&configuration.database).await;

    let server =
        zero2prod::startup::run(listener, connection_pool.clone()).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    //We return the application address to the caller
    TestApp {
        address,
        db_pool: connection_pool,
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // create the database

    let mut connection =
        PgConnection::connect(config.connection_string_without_db().expose_secret())
            .await
            .expect("Failed to connect to Postgres.");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    //Migrate database
    let connection_pool = PgPool::connect(config.connection_string().expose_secret())
        .await
        .expect("Failed to connecto to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}
