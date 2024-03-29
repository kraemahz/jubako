use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use subseq_util::{
    Router,
    BaseConfig,
    InnerConfig,
    tracing::setup_tracing,
    tables::{establish_connection_pool},
    api::{sessions, handle_rejection, init_session_store},
    oidc::{init_client_pool, IdentityProvider, OidcCredentials}
};
use tokio::sync::broadcast;
use warp::{Filter, reject::Rejection};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    conf: PathBuf,
}

const JUBAKO_PORT: u16 = 8447;

#[tokio::main]
async fn main() {
    setup_tracing("jubako");
    let args = Args::parse();
    let conf_file = File::open(args.conf).expect("Could not open file");
    let conf: BaseConfig = serde_json::from_reader(conf_file).expect("Reading config failed");
    let conf: InnerConfig = conf.try_into().expect("Could not fetch all secrets from environment");
    let openai_api_key = env::var("OPENAI_API_KEY").expect("Need to specify OPENAI_API_KEY");
    let auth_token: Arc<str> = openai_api_key.into();

    // Database and events
    let database_url = conf.database.db_url("jubako");
    let prism_url = jubako::events::prism_url(&conf.prism.host, conf.prism.port);
    let pool = establish_connection_pool(&database_url).await;
    let pool = Arc::new(pool);

    // OIDC
    init_client_pool(&conf.tls.ca_path);
    let redirect_url = format!("https://localhost:{JUBAKO_PORT}/auth");
    let oidc = OidcCredentials::new(&conf.oidc.client_id,
                                    &conf.oidc.client_secret.expect("No OIDC Client Secret"),
                                    redirect_url)
        .expect("Invalid OIDC Credentials");
    let idp = IdentityProvider::new(&oidc, &conf.oidc.idp_url.to_string()).await
        .expect("Failed to establish Identity Provider connection");
    let idp = Arc::new(idp);

    // Server setup
    let session = init_session_store();
    let mut router = Router::new();

    let log_requests = warp::log::custom(|info| {
        tracing::info!("{} {} {} {}",
                       info.remote_addr()
                           .map(|addr| addr.to_string())
                           .unwrap_or_else(|| "???".into()),
                       info.method(),
                       info.path(),
                       info.status());
    });

    let probe = warp::path("probe")
        .and(warp::get())
        .and_then(|| async {Ok::<_, Rejection>(warp::reply::html("<html>up</html>"))});
    let frontend = warp::path::end().and(warp::fs::file("dist/index.html"));
    let ico = warp::path("favicon.ico").and(warp::fs::file("dist/favicon.ico"));
    let logo = warp::path("subseq-logo.svg").and(warp::fs::file("dist/subseq-logo.svg"));
    let assets = warp::path("assets").and(warp::fs::dir("dist/assets"));

    let routes = probe
        .or(frontend)
        .or(ico)
        .or(logo)
        .or(assets)
        .recover(handle_rejection)
        .with(log_requests);

    jubako::events::emit_events(&prism_url, router, pool.clone());

    warp::serve(routes)
        .tls()
        .cert_path(&conf.tls.cert_path)
        .key_path(&conf.tls.key_path)
        .run(([127, 0, 0, 1], JUBAKO_PORT)).await;
}
