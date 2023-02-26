use database::DB_NAME;
use mockall_double::double;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, IntoMakeService},
    Json, Router,
};
use dotenvy::dotenv;
use mongodb::bson::doc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer, set_header::SetResponseHeaderLayer, timeout::TimeoutLayer, trace::TraceLayer,
    ServiceBuilderExt,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[double]
use database::AppDatabase;

mod database;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or("axum-testing=debug".into());
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let app = create_app().await;
    tracing::debug!("Starting the app in: {addr}");
    axum::Server::bind(&addr).serve(app).await.unwrap();
}

async fn create_app() -> IntoMakeService<Router> {
    let timeout_layer = TimeoutLayer::new(Duration::from_secs(10));
    let cors_layer = CorsLayer::permissive();
    let server_header_value = HeaderValue::from_static("axum_testing");
    let set_res_header_layer =
        SetResponseHeaderLayer::if_not_present(header::SERVER, server_header_value);
    let middleware = ServiceBuilder::new()
        .layer(timeout_layer)
        .layer(cors_layer)
        .layer(set_res_header_layer)
        .map_response_body(axum::body::boxed)
        .layer(TraceLayer::new_for_http())
        .compression()
        .into_inner();

    let uri = std::env::var("MONGODB_URI").expect("MONGODB_URI not found in .env file");
    let db = AppDatabase::new(uri.as_str()).await.unwrap();
    let db = Arc::new(db);
    let app: Router<(), Body> = Router::new()
        .route("/user", get(get_user_handler).post(create_user_handler))
        .layer(middleware)
        .with_state(db);

    app.into_make_service()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u32,
    name: String,
    phone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(rename = "isActive")]
    is_active: bool,
}

async fn get_user_handler(State(database): State<Arc<AppDatabase>>) -> impl IntoResponse {
    let coll_name = "users";
    let filter = Some(doc! {"id": 76});
    let result = database
        .find_one::<User>(DB_NAME, coll_name, filter, None)
        .await
        .unwrap();
    (StatusCode::OK, Json(result.unwrap()))
}

async fn create_user_handler(
    State(database): State<Arc<AppDatabase>>,
    Json(payload): Json<User>,
) -> impl IntoResponse {
    println!("create_user_handler called");
    let coll_name = "users";
    let result = database
        .insert_one(DB_NAME, coll_name, &payload, None)
        .await;
    if result.is_err() {
        tracing::debug!("{:?}", result.err().unwrap());
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "Unexpected error"})),
        );
    }
    let result = result.unwrap();
    (
        StatusCode::OK,
        Json(json!({"success": true, "insertedID": result.inserted_id })),
    )
}

#[cfg(test)]
mod tests {
    use crate::database::InsertOneResult;

    use super::*;
    use axum::http::Request;
    use axum::http::StatusCode;
    use mockall::predicate::eq;
    use mockall::predicate::function;
    use mongodb::bson::oid::ObjectId;
    use mongodb::options::FindOneOptions;
    use mongodb::options::InsertOneOptions;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_create_user_handler() {
        let user = User {
            id: 200075,
            name: "Sibaprasad".to_string(),
            phone: "56565656".to_string(),
            email: None,
            is_active: true,
        };
        let coll_name = "users";
        let insert_one_result = InsertOneResult {
            inserted_id: ObjectId::new().to_hex(),
        };
        let is_none = function(|x: &Option<InsertOneOptions>| x.is_none());
        let mut mock_db = AppDatabase::default();
        mock_db
            .expect_insert_one::<User>()
            .with(eq(DB_NAME), eq(coll_name), eq(user.clone()), is_none)
            .times(1)
            .returning(move |_, _, _, _| Ok(insert_one_result.clone()));
        let db = Arc::new(mock_db);
        let app = Router::new()
            .route("/", get(create_user_handler))
            .with_state(db);
        let stringified = serde_json::to_string(&user).unwrap();
        let req = Request::builder()
            .uri("/")
            .header("Content-Type", "application/json")
            .body(stringified)
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_user_handler() {
        let user = User::default();
        let coll_name = "users";
        let filter = Some(doc! {"id": 76});
        let is_none = function(|x: &Option<FindOneOptions>| x.is_none());
        let mut mock_db = AppDatabase::default();
        mock_db
            .expect_find_one::<User>()
            .with(eq(DB_NAME), eq(coll_name), eq(filter), is_none)
            .times(1)
            .returning(move |_, _, _, _| Ok(Some(user.clone())));
        let db = Arc::new(mock_db);
        let app = Router::new()
            .route("/", get(get_user_handler))
            .with_state(db);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
